/// jamo-core ラッパー
///
/// 辞書ロード + 変換呼び出し。
use std::path::{Path, PathBuf};

use jamo_core::dict::Entry;
use jamo_core::ipadic::IpaDict;
use jamo_core::viterbi::{Clause, ConvertResult, Segment};

pub struct ImeEngine {
    dict: Option<IpaDict>,
}

impl ImeEngine {
    /// 辞書をロード
    pub fn load(dict_path: &Path) -> Result<Self, String> {
        let dict = IpaDict::load_binary(dict_path).map_err(|e| e.to_string())?;
        Ok(Self { dict: Some(dict) })
    }

    /// デフォルトパスから辞書をロード
    pub fn load_default() -> Self {
        if let Some(path) = Self::find_dict_path() {
            if let Ok(engine) = Self::load(&path) {
                return engine;
            }
        }
        Self { dict: None }
    }

    /// 辞書パスを探す
    fn find_dict_path() -> Option<PathBuf> {
        #[cfg(windows)]
        {
            if let Some(path) = Self::dict_path_from_registry() {
                if path.exists() {
                    return Some(path);
                }
            }
            if let Some(path) = Self::dict_path_next_to_dll() {
                if path.exists() {
                    return Some(path);
                }
            }
            if let Some(path) = Self::dict_path_appdata() {
                if path.exists() {
                    return Some(path);
                }
            }
        }
        None
    }

    #[cfg(windows)]
    fn dict_path_from_registry() -> Option<PathBuf> {
        use windows::Win32::System::Registry::*;
        use windows::core::w;

        unsafe {
            let mut hkey = Default::default();
            let result = RegOpenKeyExW(
                HKEY_CURRENT_USER,
                w!("Software\\Jamo"),
                0,
                KEY_READ,
                &mut hkey,
            );
            if result.is_err() {
                return None;
            }

            let mut buf = [0u16; 260];
            let mut buf_len = (buf.len() * 2) as u32;
            let mut reg_type = REG_VALUE_TYPE(0);
            let result = RegQueryValueExW(
                hkey,
                w!("DictPath"),
                None,
                Some(&mut reg_type),
                Some(buf.as_mut_ptr() as *mut u8),
                Some(&mut buf_len),
            );
            RegCloseKey(hkey).ok();

            if result.is_err() {
                return None;
            }

            let len = (buf_len as usize / 2).saturating_sub(1);
            let path = String::from_utf16_lossy(&buf[..len]);
            Some(PathBuf::from(path))
        }
    }

    #[cfg(windows)]
    fn dict_path_next_to_dll() -> Option<PathBuf> {
        use crate::globals::get_dll_instance;
        use windows::Win32::Foundation::MAX_PATH;
        use windows::Win32::System::LibraryLoader::GetModuleFileNameW;

        let hmodule = get_dll_instance();
        let mut path = [0u16; MAX_PATH as usize];
        let len = unsafe { GetModuleFileNameW(Some(hmodule), &mut path) } as usize;
        if len == 0 {
            return None;
        }
        let dll_path = PathBuf::from(String::from_utf16_lossy(&path[..len]));
        dll_path.parent().map(|dir| dir.join("ipadic.dict"))
    }

    #[cfg(windows)]
    fn dict_path_appdata() -> Option<PathBuf> {
        std::env::var("APPDATA")
            .ok()
            .map(|appdata| PathBuf::from(appdata).join("Jamo").join("ipadic.dict"))
    }

    /// かな漢字変換
    pub fn convert(&self, hiragana: &str, n: usize) -> ConvertResult {
        match &self.dict {
            Some(dict) => jamo_core::convert_nbest(hiragana, dict, dict, n),
            None => {
                // 辞書なし: ひらがなをそのまま1文節として返す
                ConvertResult {
                    segments: vec![Segment {
                        surface: hiragana.to_string(),
                        yomi: hiragana.to_string(),
                        candidates: vec![hiragana.to_string()],
                        pos_type: 0,
                    }],
                    clauses: vec![Clause {
                        surface: hiragana.to_string(),
                        yomi: hiragana.to_string(),
                        candidates: vec![hiragana.to_string()],
                    }],
                }
            }
        }
    }

    /// 文節伸縮して再変換
    pub fn resize(
        &self,
        input: &str,
        boundaries: &[usize],
        clause: usize,
        delta: i32,
        n: usize,
    ) -> Option<ConvertResult> {
        let dict = self.dict.as_ref()?;
        let new_boundaries = jamo_core::resize_boundaries(input, boundaries, clause, delta)?;
        Some(jamo_core::convert_with_boundaries(
            input,
            &new_boundaries,
            dict,
            dict,
            n,
        ))
    }

    /// 予測変換
    pub fn predict(&self, prefix: &str, limit: usize) -> Vec<Entry> {
        match &self.dict {
            Some(dict) => dict.prefix_search(prefix, limit),
            None => vec![],
        }
    }
}
