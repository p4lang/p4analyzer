extern crate analyzer_core;
use analyzer_abstractions::lsp_types::{Position, TextDocumentContentChangeEvent, Range};
use analyzer_core::lsp_position_struct::LspPos;

#[test]
fn test_parse_file() {
    let file = "".to_string();
    let lsp = LspPos::parse_file(&file.clone());
    let ranges = Vec::new();
    assert_eq!(lsp.get_ranges(), ranges);

    let file = "\n".to_string();
    let lsp = LspPos::parse_file(&file.clone());
    let ranges = vec![0..0];
    assert_eq!(lsp.get_ranges(), ranges);

    let file = "0".to_string();
    let lsp = LspPos::parse_file(&file.clone());
    let ranges = vec![0..0];
    assert_eq!(lsp.get_ranges(), ranges);

    let file = "0\n".to_string();
    let lsp = LspPos::parse_file(&file.clone());
    let ranges = vec![0..1];
    assert_eq!(lsp.get_ranges(), ranges);

    let file = "012\n456\n\n9".to_string();
    let lsp = LspPos::parse_file(&file.clone());
    let ranges = vec![0..3, 4..7, 8..8, 9..9];
    assert_eq!(lsp.get_ranges(), ranges);

    let file = "012\n456\n\n9\n\n\n".to_string();
    let lsp = LspPos::parse_file(&file.clone());
    let ranges = vec![0..3, 4..7, 8..8, 9..10, 11..11, 12..12];
    assert_eq!(lsp.get_ranges(), ranges);
}

#[test]
fn test_byte_to_lsp() {
    let file = "012\n456\n\n9\nbcde\n".to_string();
    let lsp = LspPos::parse_file(&file.clone());
    
    let res = lsp.byte_to_lsp(0);
    assert_eq!(res, Position{ line: 0, character: 0 });
    let res = lsp.byte_to_lsp(1);
    assert_eq!(res, Position{ line: 0, character: 1 });

    let res = lsp.byte_to_lsp(4);
    assert_eq!(res, Position{ line: 1, character: 0 });
    let res = lsp.byte_to_lsp(6);
    assert_eq!(res, Position{ line: 1, character: 2 });

    let res = lsp.byte_to_lsp(8);
    assert_eq!(res, Position{ line: 2, character: 0 });

    let res = lsp.byte_to_lsp(9);
    assert_eq!(res, Position{ line: 3, character: 0 });
    let res = lsp.byte_to_lsp(10);
    assert_eq!(res, Position{ line: 3, character: 1 });

    let res = lsp.byte_to_lsp(15);  // last byte
    assert_eq!(res, Position{ line: 4, character: 4 });
    // boundary check
    let res = lsp.byte_to_lsp(16);  // Byte out of range, so return highest LSP position
    assert_eq!(res, Position{ line: 4, character: 4 });
}

#[test]
fn test_lsp_to_byte() {
    let file = "012\n456\n\n9\nbcde\n".to_string();
    let lsp = LspPos::parse_file(&file.clone());

    let res = lsp.lsp_to_byte(&Position{ line: 0, character: 0 });
    assert_eq!(res, 0);
    let res = lsp.lsp_to_byte(&Position{ line: 0, character: 1 });
    assert_eq!(res, 1);

    let res = lsp.lsp_to_byte(&Position{ line: 1, character: 0 });
    assert_eq!(res, 4);
    let res = lsp.lsp_to_byte(&Position{ line: 1, character: 3 });
    assert_eq!(res, 7);

    let res = lsp.lsp_to_byte(&Position{ line: 2, character: 0 });
    assert_eq!(res, 8);

    let res = lsp.lsp_to_byte(&Position{ line: 4, character: 4 });  // last lsp position
    assert_eq!(res, 15);

    let res = lsp.lsp_to_byte(&Position{ line: 0, character: 4 });   // character doesn't exist
    assert_eq!(res, 3);
    let res = lsp.lsp_to_byte(&Position{ line: 5, character: 0 });   // line doesn't exist
    assert_eq!(res, 15);
}

#[test]
fn soundness_test() {
    let file = "012\n456\n\n9\nbcde\n".to_string();
    let lsp = LspPos::parse_file(&file.clone());

    // round trip starting at byte
    let b = 5;
    assert_eq!(b, lsp.lsp_to_byte(&lsp.byte_to_lsp(b)));

    // round trip starting at LSP
    let p = Position{ line: 1, character: 3 };
    assert_eq!(p, lsp.byte_to_lsp(lsp.lsp_to_byte(&p)));
}

// helper function
fn change_event((l1, c1): (u32, u32), (l2, c2): (u32, u32), t: String ) -> TextDocumentContentChangeEvent {
    TextDocumentContentChangeEvent {
        range: Some(Range::new(Position::new(l1, c1), Position::new(l2,c2))),
        range_length: None, // depreciated
        text: t,
    }
}

#[test]
fn test_lazy_add() {
    let original = "012\n456\n\n9\nbcde\n".to_string();    // Test String
    let original_lsp = LspPos::parse_file(&original.clone());   // Create default LspPos
    
    // Single line
    // start of line
    let mut lsp = original_lsp.clone(); // Clone test LspPos 
    let event = change_event((1,0), (1,2), "x".into());    // Test Event
    lsp.lazy_add(&event);   // Run Event
    
    let expected_lsp = LspPos::parse_file(&"012\nx6\n\n9\nbcde\n".to_string());    // pass string that represents event changes
    assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());    // Compare ranges data (should be same)

    // end of line
    let mut lsp = original_lsp.clone();
    let event = change_event((1,2), (2,0), "x".into());
    lsp.lazy_add(&event);
    let expected_lsp = LspPos::parse_file(&"012\n45x\n9\nbcde\n".to_string());
    assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());

    // entire line change
    let mut lsp = original_lsp.clone();
    let event = change_event((1,0), (2,0), "x".into());
    lsp.lazy_add(&event);
    let expected_lsp = LspPos::parse_file(&"012\nx\n9\nbcde\n".to_string());
    assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());

    //entire line deleted
    let mut lsp = original_lsp.clone();
    let event = change_event((1,0), (2,0), "".into());
    lsp.lazy_add(&event);
    let expected_lsp = LspPos::parse_file(&"012\n\n9\nbcde\n".to_string());
    assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());

    // Checking diff functionality 
    // addition smaller than deletion (also testing empty text)
    let mut lsp = original_lsp.clone();
    let event = change_event((1,0), (1,2), "".into());
    lsp.lazy_add(&event);
    let expected_lsp = LspPos::parse_file(&"012\n6\n\n9\nbcde\n".to_string());
    assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());

    // addition same size as than deletion
    let mut lsp = original_lsp.clone();
    let event = change_event((1,0), (1,2), "xy".into());
    lsp.lazy_add(&event);
    let expected_lsp = LspPos::parse_file(&"012\nxy6\n\n9\nbcde\n".to_string());
    assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());

    // addition bigger than deletion
    let mut lsp = original_lsp.clone();
    let event = change_event((1,0), (1,2), "xyz".into());
    lsp.lazy_add(&event);
    let expected_lsp = LspPos::parse_file(&"012\nxyz6\n\n9\nbcde\n".to_string());
    assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());

    // Multiple lines cases
    // delete multiple lines partially
    let mut lsp = original_lsp.clone();
    let event = change_event((3,0), (4,2), "".into());
    lsp.lazy_add(&event);
    let expected_lsp = LspPos::parse_file(&"012\n456\n\nde\n".to_string());
    assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());

    // delete multiple lines
    let mut lsp = original_lsp.clone();
    let event = change_event((1,0), (3,1), "".into());
    lsp.lazy_add(&event);
    let expected_lsp = LspPos::parse_file(&"012\n\nbcde\n".to_string());
    assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());

    // Changes in place indicates only addition
    let mut lsp = original_lsp.clone();
    let event = change_event((1,1), (1,1), "x".into());
    lsp.lazy_add(&event);
    let expected_lsp = LspPos::parse_file(&"012\n4x56\n\n9\nbcde\n".to_string()); 
    assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());

    let mut lsp = original_lsp.clone();
    let event = change_event((2,0), (2,0), "hello".into());
    lsp.lazy_add(&event);
    let expected_lsp = LspPos::parse_file(&"012\n456\nhello\n9\nbcde\n".to_string()); 
    assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());

    // Corner Cases
    // No Range provide
    let mut lsp = original_lsp.clone();
    let event = TextDocumentContentChangeEvent {
        range: None,
        range_length: None, // depreciated
        text: "hello\n".to_string(),
    };
    lsp.lazy_add(&event);
    let expected_lsp = LspPos::parse_file(&"hello\n".to_string());
    assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());

    // Boundary check on Range
    let mut lsp = original_lsp.clone();
    let event = change_event((7,0), (7,0), "xyz".into()); // line doesn't exist
    lsp.lazy_add(&event);
    let expected_lsp = LspPos::parse_file(&"012\n456\n\n9\nbcde\nxyz".to_string()); // add to end of file
    assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());

    let mut lsp = original_lsp.clone();
    let event = change_event((1,10), (1,15), "xyz".into()); // character doesn't exist
    lsp.lazy_add(&event);
    let expected_lsp = LspPos::parse_file(&"012\n456xyz\n\n9\nbcde\n".to_string()); // add to end of line
    assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());

    let mut lsp = original_lsp.clone();
    let event = change_event((1,3), (1,0), "xyz".into()); // Range end is smaller than start, produce panic
    let res = std::panic::catch_unwind(move|| lsp.lazy_add(&event));   // catch panic
    assert!(res.is_err());  // make sure it paniced
}
