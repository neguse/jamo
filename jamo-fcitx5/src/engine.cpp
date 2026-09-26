// fcitx5 の入力メソッド。かな入力・変換・候補選択・文字種変換の状態は jamo の
// Session が持ち、ここはキー → Session 操作の対応と fcitx5 への表示だけを担う。
// キー割当は lite-dm2x の ime.lua (MS-IME 準拠) に揃える。IME の on/off は
// fcitx5 の入力メソッド切替 (keyboard-us ↔ jamo) に任せる。
#include <jamo.h>

#include <chrono>
#include <cstdlib>
#include <filesystem>
#include <memory>
#include <string>
#include <fcitx-utils/key.h>
#include <fcitx-utils/log.h>
#include <fcitx-utils/utf8.h>
#include <fcitx/addonfactory.h>
#include <fcitx/addoninstance.h>
#include <fcitx/addonmanager.h>
#include <fcitx/candidatelist.h>
#include <fcitx/inputcontext.h>
#include <fcitx/inputcontextproperty.h>
#include <fcitx/inputmethodengine.h>
#include <fcitx/inputpanel.h>
#include <fcitx/instance.h>
#include <fcitx/text.h>

namespace fcitx {
namespace {

FCITX_DEFINE_LOG_CATEGORY(jamo_log, "jamo");
#define JAMO_INFO() FCITX_LOGC(jamo_log, Info)
#define JAMO_ERROR() FCITX_LOGC(jamo_log, Error)

// 文節ごとの候補数 (辞書候補の上限)、候補一覧の 1 ページ件数
constexpr uint32_t kNBest = 9;
constexpr int kPageSize = 9;
constexpr auto kSaveInterval = std::chrono::seconds(30);

namespace fs = std::filesystem;

fs::path dataHome() {
    if (const char *xdg = std::getenv("XDG_DATA_HOME"); xdg && *xdg) {
        return fs::path(xdg) / "jamo";
    }
    const char *home = std::getenv("HOME");
    return fs::path(home ? home : "") / ".local/share/jamo";
}

fs::path findDict() {
    if (const char *env = std::getenv("JAMO_DICT"); env && *env) {
        return env;
    }
    for (const char *name : {"mozc.dict", "ipadic.dict"}) {
        if (auto p = dataHome() / name; fs::exists(p)) {
            return p;
        }
    }
    return {};
}

std::string str(const char *s) { return s ? s : ""; }

// 合成を始める文字 (ime.lua の starts_composition と同じ)。数字は「8がつ」を
// 1 文節で変換するため preedit に入れる。括弧などは合成外なら素通し
bool startsComposition(uint32_t c) {
    return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') ||
           (c >= '0' && c <= '9') || std::string_view(",.-?!/[]~").find(
                                         static_cast<char>(c)) !=
                                         std::string_view::npos;
}

// 修飾キー (Ctrl/Alt/Super) なしで打たれた文字。文字でなければ 0
uint32_t typedChar(const Key &key) {
    if (key.states().testAny(
            KeyStates{KeyState::Ctrl, KeyState::Alt, KeyState::Super})) {
        return 0;
    }
    return Key::keySymToUnicode(key.sym());
}

class JamoEngine;

class JamoState : public InputContextProperty {
public:
    JamoState(JamoEngine *engine, InputContext *ic);
    ~JamoState() override { jamo_session_free(session_); }

    void keyEvent(KeyEvent &event);
    void commit();
    void clear();
    void selectCandidate(int index);

private:
    bool composing() { return !str(jamo_session_preedit(session_)).empty(); }
    bool converting() const {
        return jamo_session_state(session_) == JAMO_STATE_CONVERTING;
    }
    bool handleTransform(const Key &key);
    bool handleInput(const Key &key);
    bool handleConverting(const Key &key);
    void moveCandidate(int delta);
    void commitString(const std::string &text);
    void update();

    JamoEngine *engine_;
    InputContext *ic_;
    jamo_session_t *session_;
    int32_t lastTransform_ = -1;
    // 候補一覧を出しているか。最初の Space は変換のみで、次の候補移動から出す
    // (Hyprland / sway は preedit と同時に出した popup を古いカーソル位置に置く)
    bool listShown_ = false;
};

class JamoCandidate : public CandidateWord {
public:
    JamoCandidate(JamoState *state, int index, std::string text)
        : CandidateWord(Text(std::move(text))), state_(state), index_(index) {}
    void select(InputContext *) const override {
        state_->selectCandidate(index_);
    }

private:
    JamoState *state_;
    int index_;
};

class JamoEngine : public InputMethodEngineV2 {
public:
    // 辞書のロードは jamo に初めて切り替えたとき (OnDemand のアドオン生成時)
    explicit JamoEngine(Instance *instance) {
        loadEngine();
        instance->inputContextManager().registerProperty("jamoState",
                                                         &factory_);
    }
    ~JamoEngine() override {
        // セッションはエンジンを参照するので先に消す
        factory_.unregister();
        if (engine_) {
            saveUserData();
            jamo_engine_free(engine_);
        }
    }

    void keyEvent(const InputMethodEntry &, KeyEvent &event) override {
        if (!engine_) {
            return;
        }
        event.inputContext()->propertyFor(&factory_)->keyEvent(event);
    }
    void deactivate(const InputMethodEntry &,
                    InputContextEvent &event) override {
        if (!engine_) {
            return;
        }
        auto *state = event.inputContext()->propertyFor(&factory_);
        // フォーカス喪失時は fcitx5 がクライアント preedit を確定済み
        if (event.type() == EventType::InputContextFocusOut) {
            state->clear();
        } else {
            state->commit();
        }
    }
    void reset(const InputMethodEntry &, InputContextEvent &event) override {
        if (!engine_) {
            return;
        }
        event.inputContext()->propertyFor(&factory_)->commit();
    }

    jamo_engine_t *engine() const { return engine_; }

    // 確定のたびに呼ぶ。書き込みは kSaveInterval に 1 回まで
    void learned() {
        auto now = std::chrono::steady_clock::now();
        if (now - lastSave_ >= kSaveInterval) {
            lastSave_ = now;
            saveUserData();
        }
    }

private:
    void loadEngine() {
        auto dict = findDict();
        if (dict.empty() || !(engine_ = jamo_engine_open(dict.c_str()))) {
            JAMO_ERROR() << "failed to load dictionary: "
                         << (dict.empty() ? dataHome() / "mozc.dict" : dict);
            return;
        }
        JAMO_INFO() << "loaded dictionary " << dict;
        if (fs::exists(userDataPath())) {
            jamo_engine_load_user_data(engine_, userDataPath().c_str());
        }
    }
    static fs::path userDataPath() { return dataHome() / "user.dat"; }
    void saveUserData() {
        std::error_code ec;
        fs::create_directories(dataHome(), ec);
        if (jamo_engine_save_user_data(engine_, userDataPath().c_str()) !=
            JAMO_OK) {
            JAMO_ERROR() << "failed to save " << userDataPath();
        }
    }

    jamo_engine_t *engine_ = nullptr;
    std::chrono::steady_clock::time_point lastSave_;
    FactoryFor<JamoState> factory_{
        [this](InputContext &ic) { return new JamoState(this, &ic); }};
};

JamoState::JamoState(JamoEngine *engine, InputContext *ic)
    : engine_(engine), ic_(ic),
      session_(jamo_session_new(engine->engine())) {}

void JamoState::keyEvent(KeyEvent &event) {
    if (event.isRelease()) {
        return;
    }
    const Key &key = event.key();
    if (!composing()) {
        if (uint32_t c = typedChar(key); startsComposition(c)) {
            lastTransform_ = -1;
            jamo_session_feed(session_, c);
            update();
            event.filterAndAccept();
        }
        return;
    }
    if (handleTransform(key) ||
        (converting() ? handleConverting(key) : handleInput(key))) {
        event.filterAndAccept();
    }
}

bool JamoState::handleTransform(const Key &key) {
    int32_t kind = -1;
    if (key.check(FcitxKey_Hiragana_Katakana)) {
        kind = lastTransform_ == JAMO_TRANSFORM_KATAKANA
                   ? JAMO_TRANSFORM_HIRAGANA
                   : JAMO_TRANSFORM_KATAKANA;
    } else if (key.check(FcitxKey_F6) || key.check(FcitxKey_u, KeyState::Ctrl)) {
        kind = JAMO_TRANSFORM_HIRAGANA;
    } else if (key.check(FcitxKey_F7) || key.check(FcitxKey_i, KeyState::Ctrl)) {
        kind = JAMO_TRANSFORM_KATAKANA;
    } else if (key.check(FcitxKey_F9) || key.check(FcitxKey_p, KeyState::Ctrl)) {
        kind = JAMO_TRANSFORM_WIDE_ALPHA;
    } else if (key.check(FcitxKey_F10) ||
               key.check(FcitxKey_t, KeyState::Ctrl)) {
        kind = JAMO_TRANSFORM_HALF_ALPHA;
    } else {
        return false;
    }
    if (jamo_session_transform(session_, kind) == JAMO_OK) {
        lastTransform_ = kind;
        update();
    }
    return true;
}

bool JamoState::handleInput(const Key &key) {
    if (key.check(FcitxKey_space) || key.check(FcitxKey_Henkan)) {
        jamo_session_convert(session_, kNBest);
        listShown_ = false;
    } else if (key.check(FcitxKey_Return) || key.check(FcitxKey_KP_Enter)) {
        commit();
        return true;
    } else if (key.check(FcitxKey_Escape)) {
        clear();
        return true;
    } else if (key.check(FcitxKey_BackSpace)) {
        jamo_session_backspace(session_);
    } else if (key.check(FcitxKey_Delete)) {
        jamo_session_delete_forward(session_);
    } else if (key.check(FcitxKey_Left)) {
        jamo_session_move_cursor(session_, -1);
    } else if (key.check(FcitxKey_Right)) {
        jamo_session_move_cursor(session_, 1);
    } else if (key.check(FcitxKey_Home)) {
        jamo_session_set_cursor(session_, 0);
    } else if (key.check(FcitxKey_End)) {
        jamo_session_set_cursor(session_, UINT32_MAX);
    } else if (key.check(FcitxKey_Up) || key.check(FcitxKey_Down)) {
        return true;
    } else if (uint32_t c = typedChar(key)) {
        jamo_session_feed(session_, c);
    } else {
        return false;
    }
    update();
    return true;
}

bool JamoState::handleConverting(const Key &key) {
    auto focus = static_cast<uint32_t>(jamo_session_focus(session_));
    auto clauses = jamo_session_clause_count(session_);
    if (key.check(FcitxKey_Down, KeyState::Ctrl)) {
        commitString(str(jamo_session_commit_prefix(session_)));
        listShown_ = false;
    } else if (key.check(FcitxKey_space) || key.check(FcitxKey_Henkan) ||
               key.check(FcitxKey_Down)) {
        moveCandidate(1);
    } else if (key.check(FcitxKey_Up)) {
        moveCandidate(-1);
    } else if (key.check(FcitxKey_Return) || key.check(FcitxKey_KP_Enter)) {
        commit();
        return true;
    } else if (key.check(FcitxKey_Escape) || key.check(FcitxKey_BackSpace)) {
        jamo_session_revert(session_);
    } else if (key.check(FcitxKey_Left, KeyState::Shift)) {
        jamo_session_resize(session_, -1, kNBest);
        listShown_ = false;
    } else if (key.check(FcitxKey_Right, KeyState::Shift)) {
        jamo_session_resize(session_, 1, kNBest);
        listShown_ = false;
    } else if (key.check(FcitxKey_Left)) {
        jamo_session_set_focus(session_, focus > 0 ? focus - 1 : 0);
        listShown_ = false;
    } else if (key.check(FcitxKey_Right)) {
        if (static_cast<int32_t>(focus) + 1 < clauses) {
            jamo_session_set_focus(session_, focus + 1);
        }
        listShown_ = false;
    } else if (listShown_ && key.sym() >= FcitxKey_1 &&
               key.sym() <= FcitxKey_9 &&
               !key.states().testAny(KeyState::SimpleMask)) {
        int page = jamo_session_selected(session_, focus) / kPageSize;
        selectCandidate(page * kPageSize + (key.sym() - FcitxKey_1));
        return true;
    } else if (uint32_t c = typedChar(key)) {
        // 変換中の文字入力は確定して次の入力を始める
        commit();
        lastTransform_ = -1;
        jamo_session_feed(session_, c);
    } else {
        return false;
    }
    update();
    return true;
}

void JamoState::moveCandidate(int delta) {
    listShown_ = true;
    auto focus = static_cast<uint32_t>(jamo_session_focus(session_));
    int n = jamo_session_candidate_count(session_, focus);
    if (n > 0) {
        int next = (jamo_session_selected(session_, focus) + delta + n) % n;
        jamo_session_select(session_, focus, next);
    }
}

void JamoState::selectCandidate(int index) {
    auto focus = static_cast<uint32_t>(jamo_session_focus(session_));
    if (jamo_session_select(session_, focus, index) == JAMO_OK) {
        update();
    }
}

void JamoState::commit() {
    if (composing()) {
        commitString(str(jamo_session_commit(session_)));
    }
    update();
}

void JamoState::clear() {
    jamo_session_reset(session_);
    update();
}

void JamoState::commitString(const std::string &text) {
    if (text.empty()) {
        return;
    }
    // preedit を消してから確定する (クライアントに二重表示させない)
    ic_->inputPanel().reset();
    ic_->updatePreedit();
    ic_->commitString(text);
    engine_->learned();
}

void JamoState::update() {
    auto &panel = ic_->inputPanel();
    panel.reset();
    std::string preedit = str(jamo_session_preedit(session_));
    if (!preedit.empty()) {
        uint32_t from = 0, len = 0;
        jamo_session_focused_range(session_, &from, &len);
        auto byteAt = [&preedit](uint32_t chars) {
            auto end = utf8::nextNChar(preedit.begin(), chars);
            return static_cast<size_t>(end - preedit.begin());
        };
        size_t b0 = byteAt(from), b1 = byteAt(from + len);
        Text text;
        text.append(preedit.substr(0, b0), TextFormatFlag::Underline);
        if (b1 > b0) {
            text.append(preedit.substr(b0, b1 - b0),
                        {TextFormatFlag::Underline, TextFormatFlag::HighLight});
        }
        text.append(preedit.substr(b1), TextFormatFlag::Underline);
        text.setCursor(static_cast<int>(b0));
        if (ic_->capabilityFlags().test(CapabilityFlag::Preedit)) {
            panel.setClientPreedit(text);
        } else {
            panel.setPreedit(text);
        }
    }
    auto focus = static_cast<uint32_t>(jamo_session_focus(session_));
    int n = converting() && listShown_
                ? jamo_session_candidate_count(session_, focus)
                : 0;
    if (n >= 2) {
        auto list = std::make_unique<CommonCandidateList>();
        list->setPageSize(kPageSize);
        list->setLayoutHint(CandidateLayoutHint::Vertical);
        list->setSelectionKey(Key::keyListFromString("1 2 3 4 5 6 7 8 9"));
        for (int i = 0; i < n; i++) {
            list->append<JamoCandidate>(
                this, i, str(jamo_session_candidate(session_, focus, i)));
        }
        int selected = jamo_session_selected(session_, focus);
        list->setPage(selected / kPageSize);
        list->setGlobalCursorIndex(selected);
        panel.setCandidateList(std::move(list));
    }
    ic_->updatePreedit();
    ic_->updateUserInterface(UserInterfaceComponent::InputPanel);
}

class JamoFactory : public AddonFactory {
public:
    AddonInstance *create(AddonManager *manager) override {
        return new JamoEngine(manager->instance());
    }
};

} // namespace
} // namespace fcitx

FCITX_ADDON_FACTORY_V2(jamo, fcitx::JamoFactory);
