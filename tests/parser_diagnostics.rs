use libxml::parser::{Parser, ParserOptions, XmlParseFailure};

fn strict() -> ParserOptions<'static> {
  ParserOptions {
    recover: false,
    no_net: true,
    ..Default::default()
  }
}

#[test]
fn owned_diagnostic_survives_later_parses() {
  let parser = Parser::default();
  let failure = parser
    .parse_string_with_diagnostics("\n<?xml version=\"1.0\"?><root/>", strict())
    .err()
    .expect("invalid declaration");
  let XmlParseFailure::ParseFailed(Some(error)) = failure else {
    panic!("expected a structured parser error");
  };
  assert_eq!(error.line, Some(2));
  assert!(error.col.is_some());
  let message = error.message.as_ref().unwrap();
  assert!(message.contains("declaration"), "{message}");
  let later = parser
    .parse_string_with_diagnostics("<root>&missing;</root>", strict())
    .err()
    .unwrap();
  assert!(later.to_string().contains("missing"));
  assert_ne!(message, &later.to_string());
  let doc = parser
    .parse_string_with_diagnostics("<root>ok</root>", strict())
    .unwrap();
  assert_eq!(doc.get_root_element().unwrap().get_content(), "ok");
  assert!(message.contains("declaration"));
}

#[test]
fn contexts_do_not_share_errors_across_threads() {
  let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
  let threads: Vec<_> = (0..8)
    .map(|i| {
      let barrier = barrier.clone();
      std::thread::spawn(move || {
        let entity = format!("missing_{i}");
        let xml = format!("<root>&{entity};</root>");
        barrier.wait();
        let failure = Parser::default()
          .parse_string_with_diagnostics(xml, strict())
          .err()
          .unwrap();
        let XmlParseFailure::ParseFailed(Some(error)) = failure else {
          panic!("expected a structured parser error");
        };
        assert!(error.message.unwrap().contains(&entity));
      })
    })
    .collect();
  for thread in threads {
    thread.join().unwrap();
  }
}

#[test]
fn invalid_encoding_names_return_errors_without_panicking() {
  let options = ParserOptions {
    encoding: Some("UTF-8\0bad"),
    ..strict()
  };
  let failure = Parser::default()
    .parse_string_with_diagnostics("<root/>", options)
    .err()
    .unwrap();
  assert!(std::error::Error::source(&failure).is_some());
  let XmlParseFailure::InvalidEncoding(error) = failure else {
    panic!("expected an encoding setup error");
  };
  assert_eq!(error.nul_position(), 5);
}

#[test]
fn xml_and_html_documents_outlive_their_parser_contexts() {
  for parser in [Parser::default(), Parser::default_html()] {
    let document = parser
      .parse_string_with_diagnostics(
        b"<html><body>caf\xe9</body></html>",
        ParserOptions {
          encoding: Some("ISO-8859-1"),
          ..strict()
        },
      )
      .unwrap();
    assert_eq!(document.get_root_element().unwrap().get_content(), "café");
  }
}
