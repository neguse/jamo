// testfrontend で fcitx5 を起動し、キー列 → 確定文字列を検証する E2E テスト
#include <filesystem>
#include <string>
#include <fcitx-utils/eventdispatcher.h>
#include <fcitx-utils/key.h>
#include <fcitx-utils/log.h>
#include <fcitx-utils/macros.h>
#include <fcitx-utils/testing.h>
#include <fcitx-utils/utf8.h>
#include <fcitx/addonmanager.h>
#include <fcitx/candidatelist.h>
#include <fcitx/inputmethodgroup.h>
#include <fcitx/inputmethodmanager.h>
#include <fcitx/inputpanel.h>
#include <fcitx/instance.h>
#include "testfrontend_public.h"
#include "testdir.h"

using namespace fcitx;

namespace {

struct Harness {
    Instance *instance;
    AddonInstance *frontend;
    ICUUID uuid;

    bool key(const Key &k) {
        return frontend->call<ITestFrontend::sendKeyEvent>(uuid, k, false);
    }
    void type(const std::string &s) {
        for (char c : s) {
            FCITX_ASSERT(key(Key(static_cast<KeySym>(c)))) << c;
        }
    }
    void expect(const std::string &s) {
        frontend->call<ITestFrontend::pushCommitExpectation>(s);
    }
    InputContext *ic() {
        return instance->inputContextManager().findByUUID(uuid);
    }
    // preedit のうち強調表示 (フォーカス文節) の部分
    std::string focused() {
        const auto &text = ic()->inputPanel().clientPreedit();
        std::string out;
        for (size_t i = 0; i < text.size(); i++) {
            if (text.formatAt(i).test(TextFormatFlag::HighLight)) {
                out += text.stringAt(i);
            }
        }
        return out;
    }
};

void run(Instance *instance) {
    auto group = instance->inputMethodManager().currentGroup();
    group.inputMethodList().clear();
    group.inputMethodList().push_back(InputMethodGroupItem("keyboard-us"));
    group.inputMethodList().push_back(InputMethodGroupItem("jamo"));
    group.setDefaultInputMethod("");
    instance->inputMethodManager().setGroup(group);

    auto *frontend = instance->addonManager().addon("testfrontend");
    Harness h{instance, frontend,
              frontend->call<ITestFrontend::createInputContext>("testapp")};
    // text-input-v3 のクライアントと同じく preedit を自前で描く
    h.ic()->setCapabilityFlags(CapabilityFlag::Preedit);
    FCITX_ASSERT(h.key(Key("Control+space")));
    FCITX_ASSERT(instance->inputMethod(h.ic()) == "jamo");

    // 空のときの Enter はアプリへ通す
    FCITX_ASSERT(!h.key(Key(FcitxKey_Return)));

    // ひらがなのまま確定
    h.expect("にほんご");
    h.type("nihongo");
    FCITX_ASSERT(h.ic()->inputPanel().clientPreedit().toString() == "にほんご")
        << h.ic()->inputPanel().clientPreedit().toString() << " / "
        << h.ic()->inputPanel().preedit().toString();
    FCITX_ASSERT(h.key(Key(FcitxKey_Return)));

    // 連文節変換
    h.expect("今日はいい天気ですね");
    h.type("kyouhaiitenkidesune");
    FCITX_ASSERT(h.key(Key(FcitxKey_space)));
    FCITX_ASSERT(h.key(Key(FcitxKey_Return)));

    // Escape で入力を破棄し、次の Enter はアプリへ通す
    h.type("abc");
    FCITX_ASSERT(h.key(Key(FcitxKey_Escape)));
    FCITX_ASSERT(h.ic()->inputPanel().clientPreedit().empty());
    FCITX_ASSERT(!h.key(Key(FcitxKey_Return)));

    // F7 でカタカナ
    h.expect("カナ");
    h.type("kana");
    FCITX_ASSERT(h.key(Key(FcitxKey_F7)));
    FCITX_ASSERT(h.key(Key(FcitxKey_Return)));

    // F10 で打鍵したローマ字に戻す
    h.expect("kana");
    h.type("kana");
    FCITX_ASSERT(h.key(Key(FcitxKey_F10)));
    FCITX_ASSERT(h.key(Key(FcitxKey_Return)));

    // Shift+← でフォーカス文節を縮める
    h.type("kyouhaiitenkidesune");
    FCITX_ASSERT(h.key(Key(FcitxKey_space)));
    auto before = h.focused();
    FCITX_ASSERT(h.key(Key("Shift+Left")));
    auto after = h.focused();
    FCITX_ASSERT(utf8::length(after) + 1 == utf8::length(before))
        << before << " -> " << after;
    h.expect(h.ic()->inputPanel().clientPreedit().toString());
    FCITX_ASSERT(h.key(Key(FcitxKey_Return)));

    // 大文字始まりは英字のまま
    h.expect("Rust");
    h.type("Rust");
    FCITX_ASSERT(h.key(Key(FcitxKey_Return)));

    // 最初の Space は変換のみ。次の Space で一覧を出して次候補へ進み、数字キーで選ぶ
    h.type("kyou");
    FCITX_ASSERT(h.key(Key(FcitxKey_space)));
    FCITX_ASSERT(!h.ic()->inputPanel().candidateList());
    FCITX_ASSERT(h.key(Key(FcitxKey_space)));
    auto list = h.ic()->inputPanel().candidateList();
    FCITX_ASSERT(list && list->size() >= 2);
    FCITX_ASSERT(h.ic()->inputPanel().clientPreedit().toString() ==
                 list->candidate(1).text().toString());
    auto first = list->candidate(0).text().toString();
    h.expect(first);
    FCITX_ASSERT(h.key(Key(FcitxKey_1)));
    FCITX_ASSERT(h.ic()->inputPanel().clientPreedit().toString() == first);
    FCITX_ASSERT(h.key(Key(FcitxKey_Return)));

    // 文節移動で一覧を閉じる
    h.type("kyouhaiitenkidesune");
    FCITX_ASSERT(h.key(Key(FcitxKey_space)));
    FCITX_ASSERT(h.key(Key(FcitxKey_Down)));
    FCITX_ASSERT(h.ic()->inputPanel().candidateList());
    FCITX_ASSERT(h.key(Key(FcitxKey_Right)));
    FCITX_ASSERT(!h.ic()->inputPanel().candidateList());
    FCITX_ASSERT(h.key(Key(FcitxKey_Escape)));
    FCITX_ASSERT(h.key(Key(FcitxKey_Escape)));

    // 変換中の文字入力は確定して次の入力を始める
    h.type("kyou");
    FCITX_ASSERT(h.key(Key(FcitxKey_space)));
    h.expect(h.ic()->inputPanel().clientPreedit().toString());
    h.expect("あ");
    h.type("a");
    FCITX_ASSERT(h.key(Key(FcitxKey_Return)));

    // 入力中に IME を切り替えると確定される
    h.expect("にほん");
    h.type("nihon");
    FCITX_ASSERT(h.key(Key("Control+space")));
    FCITX_ASSERT(instance->inputMethod(h.ic()) == "keyboard-us");

    frontend->call<ITestFrontend::destroyInputContext>(h.uuid);
}

} // namespace

int main() {
    if (!std::filesystem::exists(TESTING_DICT)) {
        FCITX_INFO() << "Skipping: dictionary not found " << TESTING_DICT;
        return 77;
    }
    setenv("JAMO_DICT", TESTING_DICT, 1);
    // 学習データは実行ごとに空から始める
    std::filesystem::remove_all(TESTING_BINARY_DIR "/test/xdg");
    setenv("XDG_DATA_HOME", TESTING_BINARY_DIR "/test/xdg", 1);
    setupTestingEnvironmentPath(TESTING_BINARY_DIR, {"bin"},
                                {TESTING_BINARY_DIR "/test"});
    char arg0[] = "testjamo";
    char arg1[] = "--disable=all";
    char arg2[] = "--enable=testim,testfrontend,jamo";
    char *argv[] = {arg0, arg1, arg2};
    Log::setLogRule("default=5,jamo=5");
    Instance instance(FCITX_ARRAY_SIZE(argv), argv);
    instance.addonManager().registerDefaultLoader(nullptr);
    instance.eventDispatcher().schedule([&instance]() {
        run(&instance);
        instance.exit();
    });
    instance.exec();
    return 0;
}
