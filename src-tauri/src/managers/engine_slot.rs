#![allow(dead_code)]

#[derive(Debug)]
pub enum EngineSlot<E> {
    Empty,

    Ready(E),

    Borrowed { model_id: Option<String> },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PutBack {
    Restored,

    ReplacedWhileBorrowed,

    UnloadedWhileBorrowed,

    SelectionChangedWhileBorrowed,
}

impl<E> Default for EngineSlot<E> {
    fn default() -> Self {
        Self::Empty
    }
}

impl<E> EngineSlot<E> {
    pub fn is_loaded(&self) -> bool {
        !matches!(self, Self::Empty)
    }

    pub fn is_borrowed(&self) -> bool {
        matches!(self, Self::Borrowed { .. })
    }

    pub fn ready(&self) -> Option<&E> {
        match self {
            Self::Ready(engine) => Some(engine),
            _ => None,
        }
    }

    pub fn borrow_engine(&mut self, model_id: Option<String>) -> Option<E> {
        if !matches!(self, Self::Ready(_)) {
            return None;
        }
        match std::mem::replace(self, Self::Borrowed { model_id }) {
            Self::Ready(engine) => Some(engine),

            _ => None,
        }
    }

    pub fn install(&mut self, engine: E) {
        *self = Self::Ready(engine);
    }

    pub fn clear(&mut self) {
        *self = Self::Empty;
    }

    pub fn put_back(
        &mut self,
        borrowed_id: &Option<String>,
        engine: E,
        current_model_id: &Option<String>,
    ) -> PutBack {
        let outcome = match self {
            Self::Borrowed { model_id } if *model_id == *borrowed_id => {
                if *current_model_id == *borrowed_id {
                    PutBack::Restored
                } else {
                    PutBack::SelectionChangedWhileBorrowed
                }
            }
            Self::Empty => PutBack::UnloadedWhileBorrowed,

            _ => PutBack::ReplacedWhileBorrowed,
        };
        match outcome {
            PutBack::Restored => self.install(engine),

            PutBack::SelectionChangedWhileBorrowed => self.clear(),

            PutBack::ReplacedWhileBorrowed | PutBack::UnloadedWhileBorrowed => {}
        }
        outcome
    }

    pub fn abandon_borrow(&mut self, borrowed_id: &Option<String>) -> bool {
        let ours = match self {
            Self::Borrowed { model_id } => *model_id == *borrowed_id,
            _ => false,
        };
        if ours {
            self.clear();
        }
        ours
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    struct Engine(&'static str);

    fn id(name: &str) -> Option<String> {
        Some(name.to_string())
    }

    #[test]
    fn a_borrowed_engine_still_counts_as_loaded() {
        let mut slot = EngineSlot::Empty;
        slot.install(Engine("large"));
        assert!(slot.is_loaded());

        let engine = slot.borrow_engine(id("large")).expect("借りられる");
        assert!(slot.is_loaded(), "貸出中も「読み込まれている」");
        assert!(slot.is_borrowed());
        assert!(slot.ready().is_none(), "中身は借り手が持っている");

        assert_eq!(
            slot.put_back(&id("large"), engine, &id("large")),
            PutBack::Restored
        );
        assert_eq!(slot.ready(), Some(&Engine("large")));
    }

    #[test]
    fn an_empty_slot_is_not_loaded() {
        let slot: EngineSlot<Engine> = EngineSlot::Empty;
        assert!(!slot.is_loaded());
        assert!(!slot.is_borrowed());
    }

    #[test]
    fn the_same_engine_cannot_be_borrowed_twice() {
        let mut slot = EngineSlot::Empty;
        slot.install(Engine("large"));
        let _first = slot.borrow_engine(id("large")).expect("1 本目は借りられる");
        assert!(
            slot.borrow_engine(id("large")).is_none(),
            "2 本目は借りられない"
        );
    }

    #[test]
    fn nothing_can_be_borrowed_from_an_empty_slot() {
        let mut slot: EngineSlot<Engine> = EngineSlot::Empty;
        assert!(slot.borrow_engine(id("large")).is_none());
        assert!(!slot.is_borrowed(), "借りられなかったら貸出中にしない");
    }

    #[test]
    fn a_model_switch_during_transcription_keeps_the_new_engine() {
        let mut slot = EngineSlot::Empty;
        slot.install(Engine("anonen-cloud:flash"));
        let old = slot
            .borrow_engine(id("anonen-cloud:flash"))
            .expect("借りられる");

        slot.install(Engine("large"));

        assert_eq!(
            slot.put_back(&id("anonen-cloud:flash"), old, &id("large")),
            PutBack::ReplacedWhileBorrowed
        );
        assert_eq!(
            slot.ready(),
            Some(&Engine("large")),
            "新しいほうが残っていること"
        );
    }

    #[test]
    fn an_unload_during_transcription_is_not_undone() {
        let mut slot = EngineSlot::Empty;
        slot.install(Engine("large"));
        let engine = slot.borrow_engine(id("large")).expect("借りられる");

        slot.clear();

        assert_eq!(
            slot.put_back(&id("large"), engine, &None),
            PutBack::UnloadedWhileBorrowed
        );
        assert!(!slot.is_loaded(), "戻して復活させない");
    }

    #[test]
    fn a_selection_change_without_a_new_engine_drops_the_old_one() {
        let mut slot = EngineSlot::Empty;
        slot.install(Engine("large"));
        let old = slot.borrow_engine(id("large")).expect("借りられる");

        assert_eq!(
            slot.put_back(&id("large"), old, &id("turbo")),
            PutBack::SelectionChangedWhileBorrowed
        );
        assert!(!slot.is_loaded(), "選択と食い違うものを残さない");
    }

    #[test]
    fn a_panicked_engine_empties_the_slot() {
        let mut slot = EngineSlot::Empty;
        slot.install(Engine("large"));
        let _broken = slot.borrow_engine(id("large")).expect("借りられる");

        assert!(slot.abandon_borrow(&id("large")));
        assert!(!slot.is_loaded(), "壊れたエンジンの跡を残さない");
    }

    #[test]
    fn a_panicked_engine_does_not_take_a_newer_one_with_it() {
        let mut slot = EngineSlot::Empty;
        slot.install(Engine("large"));
        let _broken = slot.borrow_engine(id("large")).expect("借りられる");

        slot.install(Engine("turbo"));

        assert!(!slot.abandon_borrow(&id("large")));
        assert_eq!(slot.ready(), Some(&Engine("turbo")));
    }
}
