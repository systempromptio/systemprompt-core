use systemprompt_files::documents::engine::loro::LoroEngine;
use systemprompt_files::documents::engine::loro::deck::DeckModel;
use systemprompt_files::documents::engine::loro::sheet::SheetModel;
use systemprompt_files::documents::engine::{
    DocumentEngine, EngineDoc, EngineError, ImportScope, ModelView, UpdateImport,
};
use systemprompt_files::documents::model::RichText;
use systemprompt_models::documents::EditOp;

#[derive(Debug, Clone)]
struct BodyDoc {
    text: String,
}

impl EngineDoc for BodyDoc {
    fn apply_update(&mut self, update: &[u8]) -> Result<UpdateImport, EngineError> {
        self.text = String::from_utf8(update.to_vec()).map_err(|_| EngineError::Unsupported)?;
        Ok(UpdateImport {
            peers: vec![7],
            pending: false,
        })
    }

    fn snapshot(&self) -> Result<Vec<u8>, EngineError> {
        Ok(self.text.clone().into_bytes())
    }

    fn version(&self) -> Vec<u8> {
        vec![u8::try_from(self.text.len()).unwrap_or(u8::MAX)]
    }

    fn missing_since(&self, _: &[u8]) -> Result<Option<Vec<u8>>, EngineError> {
        Ok(None)
    }

    fn export_model(&self) -> Result<ModelView, EngineError> {
        Ok(ModelView::Body(self.text.clone()))
    }

    fn fork(&self) -> Box<dyn EngineDoc> {
        Box::new(self.clone())
    }

    fn update_to_model(&self, model: &ModelView) -> Result<Option<Vec<u8>>, EngineError> {
        self.update_to_model_as(model, 0)
    }

    fn update_to_model_as(
        &self,
        model: &ModelView,
        _: u64,
    ) -> Result<Option<Vec<u8>>, EngineError> {
        match model {
            ModelView::Body(text) if *text == self.text => Ok(None),
            ModelView::Body(text) => Ok(Some(text.clone().into_bytes())),
            _ => Err(EngineError::Unsupported),
        }
    }
}

#[test]
fn model_views_name_their_kind() {
    assert_eq!(ModelView::Body(String::new()).kind(), "body");
    assert_eq!(
        ModelView::RichText(RichText::of(Vec::new())).kind(),
        "rich_text"
    );
    assert_eq!(ModelView::Sheet(SheetModel::default()).kind(), "sheet");
    assert_eq!(
        ModelView::Deck(DeckModel { slides: Vec::new() }).kind(),
        "deck"
    );
}

#[test]
fn an_engine_without_ops_support_refuses_ops_and_adopts_no_ids() {
    let mut doc = BodyDoc {
        text: "before".to_owned(),
    };
    let ops = vec![EditOp::StrReplace {
        old_str: "before".to_owned(),
        new_str: "after".to_owned(),
    }];
    assert!(matches!(
        doc.update_from_ops(&ops, 3),
        Err(EngineError::Unsupported)
    ));
    assert!(!doc.adopt_node_ids(&[]).expect("adopt"));
    assert_eq!(doc.text, "before");
}

#[test]
fn the_default_scoped_import_reports_the_whole_model_before_and_after() {
    let mut doc = BodyDoc {
        text: "before".to_owned(),
    };
    let scoped = doc.apply_update_scoped(b"after", true).expect("import");
    assert_eq!(scoped.import.peers, vec![7]);
    assert_eq!(
        scoped.scope,
        ImportScope::Full {
            before: ModelView::Body("before".to_owned()),
            after: ModelView::Body("after".to_owned()),
        }
    );
    let refused = doc.apply_update_scoped(&[0xff, 0xfe], false);
    assert!(matches!(refused, Err(EngineError::Unsupported)));
    assert_eq!(
        doc.export_model().expect("export"),
        ModelView::Body("after".to_owned())
    );
}

#[test]
fn the_loro_engine_names_itself_and_rejects_garbage_input() {
    let engine = LoroEngine;
    assert!(!engine.name().is_empty());
    assert!(!engine.wire_version().is_empty());
    assert!(engine.open(b"not a snapshot").is_err());
    assert!(engine.check_update(b"not an update", None).is_err());
}
