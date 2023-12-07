use std::collections::HashMap;
use std::convert::AsRef;

use color_eyre::eyre::Result;
use derive_deref::{Deref, DerefMut};
use serde::{de::Deserializer, Deserialize, Serialize};

const DEFAULT_WORD_CONFIG: &str = include_str!("../assets/wordbinding.toml");

#[derive(Clone, Debug, Default, Deref, DerefMut)]
pub struct WordBindings(pub HashMap<Lang, HashMap<String, String>>);

#[derive(Debug, Default)]
pub struct SelectedWordBindings(pub HashMap<String, String>);

#[allow(non_camel_case_types)]
#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Lang {
    #[default]
    en_US,
    zh_TW,
}

impl Lang {
    pub fn chose_from(lang: &str) -> Self {
        if let Some((locale, _encoding)) = lang.split_once('.') {
            log::debug!("detect locale:{locale:?}");
            match locale {
                "zh_TW" => Lang::zh_TW,
                _ => Self::default(),
            }
        } else {
            match lang {
                "zh_TW" => Lang::zh_TW,
                _ => Self::default(),
            }
        }
    }
}

impl SelectedWordBindings {
    pub fn init(lang: &str) -> SelectedWordBindings {
        let wordbindings: WordBindings = toml::from_str(DEFAULT_WORD_CONFIG).unwrap();

        let selected_wordbindings = wordbindings
            .get(&Lang::chose_from(lang))
            .map(|b| SelectedWordBindings(b.clone()))
            .unwrap_or_default();
        log::debug!("Select wordbindings:\n{selected_wordbindings:#?}");

        selected_wordbindings
    }

    pub fn get<'a>(&'a self, key: &'a str) -> &'a str {
        self.0.get(key).map(|v| v.as_ref()).unwrap_or(key)
    }
}

impl<'de> Deserialize<'de> for WordBindings {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let parsed_map = HashMap::<Lang, HashMap<String, String>>::deserialize(deserializer)?;

        let word_bindings = parsed_map.into_iter().collect();

        Ok(WordBindings(word_bindings))
    }
}
