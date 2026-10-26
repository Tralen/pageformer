//! Properties that must hold for arbitrary input.

use proptest::prelude::*;

proptest! {
    #[test]
    fn markdown_parsing_is_total(source in "\\PC{0,512}") {
        let _ = pageformer::markdown::parse(&source);
    }

    #[test]
    fn template_parsing_is_total(source in "\\PC{0,512}") {
        let _ = pageformer::template::Template::from_yaml(&source);
    }
}
