extern crate analyzer_core;
use std::time::{Duration, Instant};

use analyzer_abstractions::lsp_types::{Position, Range, TextDocumentContentChangeEvent};
use analyzer_core::lsp_file::LspFile;

#[test]
fn test_parse_file() {
	let file = "".to_string();
	let lsp = LspFile::new(&file.clone());
	let ranges: Vec<usize> = Vec::new();
	assert_eq!(*lsp.get_file(), file);
	assert_eq!(*lsp.get_ranges(), ranges);
	assert!(!lsp.get_eof());

	let file = "\n".to_string();
	let lsp = LspFile::new(&file.clone());
	let ranges = vec![0];
	assert_eq!(*lsp.get_file(), file);
	assert_eq!(*lsp.get_ranges(), ranges);
	assert!(lsp.get_eof());

	let file = "0".to_string();
	let lsp = LspFile::new(&file.clone());
	let ranges = vec![0];
	assert_eq!(*lsp.get_file(), file);
	assert_eq!(*lsp.get_ranges(), ranges);
	assert!(!lsp.get_eof());

	let file = "0\n".to_string();
	let lsp = LspFile::new(&file.clone());
	let ranges = vec![1];
	assert_eq!(*lsp.get_file(), file);
	assert_eq!(*lsp.get_ranges(), ranges);
	assert!(lsp.get_eof());

	let file = "012\n456\n\n9".to_string();
	let lsp = LspFile::new(&file.clone());
	let ranges = vec![3, 7, 8, 9];
	assert_eq!(*lsp.get_file(), file);
	assert_eq!(*lsp.get_ranges(), ranges);
	assert!(!lsp.get_eof());

	let file = "012\n456\n\n9\n\n\n".to_string();
	let lsp = LspFile::new(&file.clone());
	let ranges = vec![3, 7, 8, 10, 11, 12];
	assert_eq!(*lsp.get_file(), file);
	assert_eq!(*lsp.get_ranges(), ranges);
	assert!(lsp.get_eof());

	// unicode examples
	// 2 byte character
	let file = "ć".to_string();
	let lsp = LspFile::new(&file.clone());
	let ranges = vec![1];
	assert_eq!(*lsp.get_file(), file);
	assert_eq!(*lsp.get_ranges(), ranges);
	assert!(!lsp.get_eof());

	let file = "ć\nć\n".to_string();
	let lsp = LspFile::new(&file.clone());
	let ranges = vec![2, 5];
	assert_eq!(*lsp.get_file(), file);
	assert_eq!(*lsp.get_ranges(), ranges);
	assert!(lsp.get_eof());

	// 3 byte character
	let file = "⁺".to_string();
	let lsp = LspFile::new(&file.clone());
	let ranges = vec![2];
	assert_eq!(*lsp.get_file(), file);
	assert_eq!(*lsp.get_ranges(), ranges);
	assert!(!lsp.get_eof());

	let file = "⁺\n⁺\n".to_string();
	let lsp = LspFile::new(&file.clone());
	let ranges = vec![3, 7];
	assert_eq!(*lsp.get_file(), file);
	assert_eq!(*lsp.get_ranges(), ranges);
	assert!(lsp.get_eof());
	
	// 4 byte character
	let file = "𒀀".to_string();
	let lsp = LspFile::new(&file.clone());
	let ranges = vec![3];
	assert_eq!(*lsp.get_file(), file);
	assert_eq!(*lsp.get_ranges(), ranges);
	assert!(!lsp.get_eof());

	let file = "𒀀\n𒀀\n".to_string();
	let lsp = LspFile::new(&file.clone());
	let ranges = vec![4, 9];
	assert_eq!(*lsp.get_file(), file);
	assert_eq!(*lsp.get_ranges(), ranges);
	assert!(lsp.get_eof());
}

#[test]
fn test_byte_to_lsp() {
	let file = "012\n456\n\n9\nbcde\n".to_string();
	let lsp = LspFile::new(&file.clone());

	let res = lsp.byte_to_lsp(0);
	assert_eq!(res, Position { line: 0, character: 0 });
	let res = lsp.byte_to_lsp(1);
	assert_eq!(res, Position { line: 0, character: 1 });

	let res = lsp.byte_to_lsp(4);
	assert_eq!(res, Position { line: 1, character: 0 });
	let res = lsp.byte_to_lsp(6);
	assert_eq!(res, Position { line: 1, character: 2 });

	let res = lsp.byte_to_lsp(8);
	assert_eq!(res, Position { line: 2, character: 0 });

	let res = lsp.byte_to_lsp(9);
	assert_eq!(res, Position { line: 3, character: 0 });
	let res = lsp.byte_to_lsp(10);
	assert_eq!(res, Position { line: 3, character: 1 });

	let res = lsp.byte_to_lsp(15); // last byte
	assert_eq!(res, Position { line: 4, character: 4 });
	// boundary check
	let res = lsp.byte_to_lsp(16); // Byte out of range, so return next highest LSP position
	assert_eq!(res, Position { line: 5, character: 0 });
	// test on empty file
	let empty_lsp = LspFile::new(&"".to_string());
	let res = empty_lsp.byte_to_lsp(0); // Byte out of range
	assert_eq!(res, Position { line: 0, character: 0 });

	let res = empty_lsp.byte_to_lsp(16); // Byte further out of range
	assert_eq!(res, Position { line: 0, character: 0 });

	// unicode example
	// 4 byte character
	let file = "𒀀\n".to_string();
	let lsp = LspFile::new(&file.clone());

	// the range of the character should result in the same position
	for i in 0..4 {
		let res = lsp.byte_to_lsp(i);
		assert_eq!(res, Position { line: 0, character: 0 });
	}

	let res = lsp.byte_to_lsp(4);
	assert_eq!(res, Position { line: 0, character: 1 });
}

#[test]
fn test_lsp_to_byte() {
	let file = "012\n456\n\n9\nbcde\n".to_string();
	let lsp = LspFile::new(&file.clone());

	let res = lsp.lsp_to_byte(&Position { line: 0, character: 0 });
	assert_eq!(res, 0);
	let res = lsp.lsp_to_byte(&Position { line: 0, character: 1 });
	assert_eq!(res, 1);

	let res = lsp.lsp_to_byte(&Position { line: 1, character: 0 });
	assert_eq!(res, 4);
	let res = lsp.lsp_to_byte(&Position { line: 1, character: 3 });
	assert_eq!(res, 7);

	let res = lsp.lsp_to_byte(&Position { line: 2, character: 0 });
	assert_eq!(res, 8);

	let res = lsp.lsp_to_byte(&Position { line: 4, character: 4 }); // last lsp position
	assert_eq!(res, 15);

	let res = lsp.lsp_to_byte(&Position { line: 0, character: 4 }); // character doesn't exist
	assert_eq!(res, 3);
	let res = lsp.lsp_to_byte(&Position { line: 5, character: 0 }); // line doesn't exist
	assert_eq!(res, 16);

	// test on empty file
	let empty_lsp = LspFile::new(&"".to_string());
	let res = empty_lsp.lsp_to_byte(&Position { line: 0, character: 0 }); // lsp position out of range
	assert_eq!(res, 0);

	let res = empty_lsp.lsp_to_byte(&Position { line: 3, character: 4 }); // lsp position further out of range
	assert_eq!(res, 0);

	// unicode example
	let file = "𒀀\n".to_string();
	let lsp = LspFile::new(&file.clone());

	let res = lsp.lsp_to_byte(&Position { line: 0, character: 0 });
	assert_eq!(res, 0);
	let res = lsp.lsp_to_byte(&Position { line: 0, character: 1 });
	assert_eq!(res, 4);
}

#[test]
fn soundness_test() {
	let file = "012\n456\n\n9\nbcde\n".to_string();
	let lsp = LspFile::new(&file.clone());

	// round trip starting at byte
	let b = 5;
	assert_eq!(b, lsp.lsp_to_byte(&lsp.byte_to_lsp(b)));

	// round trip starting at LSP
	let p = Position { line: 1, character: 3 };
	assert_eq!(p, lsp.byte_to_lsp(lsp.lsp_to_byte(&p)));

	for i in 0..file.len() {
		assert_eq!(i, lsp.lsp_to_byte(&lsp.byte_to_lsp(i)));
	}
}

// helper function
fn change_event((l1, c1): (u32, u32), (l2, c2): (u32, u32), t: String) -> TextDocumentContentChangeEvent {
	TextDocumentContentChangeEvent {
		range: Some(Range::new(Position::new(l1, c1), Position::new(l2, c2))),
		range_length: None, // depreciated
		text: t,
	}
}

#[test]
fn test_lazy_add() {
	let original = "012\n456\n\n9\nbcde\n".to_string(); // Test String
	let original_lsp = LspFile::new(&original.clone()); // Create default LspPos

	// Single line
	// start of line
	let event = change_event((1, 0), (1, 2), "x".into());
	let mut lsp = original_lsp.clone();
	lsp.lazy_add(&event);
	let expect_str = "012\nx6\n\n9\nbcde\n".to_string();
	let expected_lsp = LspFile::new(&expect_str);
	assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());
	assert_eq!(expect_str, *lsp.get_file());

	// end of line
	let event = change_event((1, 2), (2, 0), "x".into());
	let mut lsp = original_lsp.clone();
	lsp.lazy_add(&event);
	let expect_str = "012\n45x\n9\nbcde\n".to_string();
	let expected_lsp = LspFile::new(&expect_str);
	assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());
	assert_eq!(expect_str, *lsp.get_file());

	// entire line change
	let event = change_event((1, 0), (2, 0), "x".into());
	let mut lsp = original_lsp.clone();
	lsp.lazy_add(&event);
	let expect_str = "012\nx\n9\nbcde\n".to_string();
	let expected_lsp = LspFile::new(&expect_str);
	assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());
	assert_eq!(expect_str, *lsp.get_file());

	// entire line deleted
	let event = change_event((1, 0), (2, 0), "".into());
	let mut lsp = original_lsp.clone();
	lsp.lazy_add(&event);
	let expect_str = "012\n\n9\nbcde\n".to_string();
	let expected_lsp = LspFile::new(&expect_str);
	assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());
	assert_eq!(expect_str, *lsp.get_file());

	// Checking diff functionality
	// addition smaller than deletion (also testing empty text)
	let mut lsp = original_lsp.clone();
	let event = change_event((1, 0), (1, 2), "".into());
	lsp.lazy_add(&event);
	let expect_str = "012\n6\n\n9\nbcde\n".to_string();
	let expected_lsp = LspFile::new(&expect_str);
	assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());
	assert_eq!(expect_str, *lsp.get_file());

	// addition same size as than deletion
	let mut lsp = original_lsp.clone();
	let event = change_event((1, 0), (1, 2), "xy".into());
	lsp.lazy_add(&event);
	let expect_str = "012\nxy6\n\n9\nbcde\n".to_string();
	let expected_lsp = LspFile::new(&expect_str);
	assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());
	assert_eq!(expect_str, *lsp.get_file());

	// addition bigger than deletion
	let mut lsp = original_lsp.clone();
	let event = change_event((1, 0), (1, 2), "xyz".into());
	lsp.lazy_add(&event);
	let expect_str = "012\nxyz6\n\n9\nbcde\n".to_string();
	let expected_lsp = LspFile::new(&expect_str);
	assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());
	assert_eq!(expect_str, *lsp.get_file());

	// Multiple lines cases
	// delete multiple lines partially
	let mut lsp = original_lsp.clone();
	let event = change_event((3, 0), (4, 2), "".into());
	lsp.lazy_add(&event);
	let expect_str = "012\n456\n\nde\n".to_string();
	let expected_lsp = LspFile::new(&expect_str);
	assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());
	assert_eq!(expect_str, *lsp.get_file());

	// delete multiple lines
	let event = change_event((1, 0), (3, 0), "".into());
	let mut lsp = original_lsp.clone();
	lsp.lazy_add(&event);
	let expect_str = "012\n9\nbcde\n".to_string();
	let expected_lsp = LspFile::new(&expect_str);
	assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());
	assert_eq!(expect_str, *lsp.get_file());

	// Change multiple lines exact
	let event = change_event((1, 0), (4, 0), "x\ny".into());
	let mut lsp = original_lsp.clone();
	lsp.lazy_add(&event);
	let expect_str = "012\nx\nybcde\n".to_string();
	let expected_lsp = LspFile::new(&expect_str);
	assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());
	assert_eq!(expect_str, *lsp.get_file());

	// Change multiple partial lines
	let mut lsp = original_lsp.clone();
	let event = change_event((1, 1), (4, 1), "x\ny".into());
	lsp.lazy_add(&event);
	let expect_str = "012\n4x\nycde\n".to_string();
	let expected_lsp = LspFile::new(&expect_str);
	assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());
	assert_eq!(expect_str, *lsp.get_file());

	// Changes in place indicates (only acts as addition)
	let mut lsp = original_lsp.clone();
	let event = change_event((1, 1), (1, 1), "x".into());
	lsp.lazy_add(&event);
	let expect_str = "012\n4x56\n\n9\nbcde\n".to_string();
	let expected_lsp = LspFile::new(&expect_str);
	assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());
	assert_eq!(expect_str, *lsp.get_file());

	let mut lsp = original_lsp.clone();
	let event = change_event((2, 0), (2, 0), "hello".into());
	lsp.lazy_add(&event);
	let expect_str = "012\n456\nhello\n9\nbcde\n".to_string();
	let expected_lsp = LspFile::new(&expect_str);
	assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());
	assert_eq!(expect_str, *lsp.get_file());

	// Changes That include '\n'
	// insert
	let mut lsp = original_lsp.clone();
	let event = change_event((2, 0), (2, 0), "\n".into());
	lsp.lazy_add(&event);
	let expect_str = "012\n456\n\n\n9\nbcde\n".to_string();
	let expected_lsp = LspFile::new(&expect_str);
	assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());
	assert_eq!(expect_str, *lsp.get_file());

	// replace
	let mut lsp = original_lsp.clone();
	let event = change_event((1, 3), (2, 0), "\n".into());
	lsp.lazy_add(&event);
	let expect_str = "012\n456\n\n9\nbcde\n".to_string();
	let expected_lsp = LspFile::new(&expect_str);
	assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());
	assert_eq!(expect_str, *lsp.get_file());

	// multiple
	let mut lsp = original_lsp.clone();
	let event = change_event((2, 0), (2, 0), "\n\n\n".into());
	lsp.lazy_add(&event);
	let expect_str = "012\n456\n\n\n\n\n9\nbcde\n".to_string();
	let expected_lsp = LspFile::new(&expect_str);
	assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());
	assert_eq!(expect_str, *lsp.get_file());

	// replace multiple
	let mut lsp = original_lsp.clone();
	let event = change_event((1, 3), (2, 0), "\n\n\n".into());
	lsp.lazy_add(&event);
	let expect_str = "012\n456\n\n\n\n9\nbcde\n".to_string();
	let expected_lsp = LspFile::new(&expect_str);
	assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());
	assert_eq!(expect_str, *lsp.get_file());

	// Corner Cases
	// No Range provide
	let mut lsp = original_lsp.clone();
	let event = TextDocumentContentChangeEvent {
		range: None,
		range_length: None, // depreciated
		text: "hello\n".to_string(),
	};
	lsp.lazy_add(&event);
	let expected_lsp = LspFile::new(&"hello\n".to_string());
	assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());

	// Empty file
	let mut lsp = LspFile::new(&"".to_string());
	let event = change_event((0, 0), (0, 0), "hello".into());
	lsp.lazy_add(&event);
	let expect_str = "hello".to_string();
	let expected_lsp = LspFile::new(&expect_str);
	assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());
	assert_eq!(expect_str, *lsp.get_file());

	let mut lsp = LspFile::new(&"".to_string());
	let event = change_event((1, 2), (3, 4), "hello".into());
	lsp.lazy_add(&event);
	let expect_str = "hello".to_string();
	let expected_lsp = LspFile::new(&expect_str);
	assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());
	assert_eq!(expect_str, *lsp.get_file());

	// changes to start of file
	let mut lsp = original_lsp.clone();
	let event = change_event((0, 0), (1, 0), "xyz".into());
	lsp.lazy_add(&event);
	let expect_str = "xyz456\n\n9\nbcde\n".to_string(); // change to start of file
	let expected_lsp = LspFile::new(&expect_str);
	assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());
	assert_eq!(expect_str, *lsp.get_file());

	// changes to end of file
	let mut lsp = original_lsp.clone();
	let event = change_event((4, 0), (6, 0), "xyz".into());
	lsp.lazy_add(&event);
	let expect_str = "012\n456\n\n9\nxyz".to_string(); // change to end of file
	let expected_lsp = LspFile::new(&expect_str);
	assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());
	assert_eq!(expect_str, *lsp.get_file());

	// changes to whole of file
	let mut lsp = original_lsp.clone();
	let event = change_event((0, 0), (5, 0), "xyz".into());
	lsp.lazy_add(&event);
	let expect_str = "xyz".to_string(); // change whole file
	let expected_lsp = LspFile::new(&expect_str);
	assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());
	assert_eq!(expect_str, *lsp.get_file());

	let mut lsp = original_lsp.clone();
	let event = change_event((0, 0), (5, 0), "".into());
	lsp.lazy_add(&event);
	let expect_str = "".to_string();
	let expected_lsp = LspFile::new(&expect_str);
	assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());
	assert_eq!(expect_str, *lsp.get_file());

	// Boundary check on Range
	let mut lsp = original_lsp.clone();
	let event = change_event((7, 0), (7, 0), "xyz".into()); // line doesn't exist
	lsp.lazy_add(&event);
	let expect_str = "012\n456\n\n9\nbcde\nxyz".to_string(); // add to end of file
	let expected_lsp = LspFile::new(&expect_str);
	assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());
	assert_eq!(expect_str, *lsp.get_file());

	let mut lsp = original_lsp.clone();
	let event = change_event((1, 10), (1, 15), "xyz".into()); // character doesn't exist
	lsp.lazy_add(&event);
	let expect_str = "012\n456xyz\n\n9\nbcde\n".to_string(); // add to end of line
	let expected_lsp = LspFile::new(&expect_str);
	assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());
	assert_eq!(expect_str, *lsp.get_file());

	let mut lsp = original_lsp.clone();
	let event = change_event((1, 1), (1, 0), "xyz".into()); // Range end is smaller than start, produce panic
	let res = std::panic::catch_unwind(move || lsp.lazy_add(&event)); // catch panic
	assert!(res.is_err()); // make sure it paniced

	// for unicode tests look at exhaustive_lazy_add()
	// too many unique posibilites, so programmably test it
}

#[test]
fn exhaustive_lazy_add() {
	// test strings
	let test_files = [
		"012\n456\n\n9\nbcde\n".to_string(), // end in '\n'
		"\n012\n456\n\n9\nbcdef".to_string(),  // doesn't end in '\n'
		"0\n2\nć\nć\n⁺\n⁺\n𒀀\n𒀀\n".to_string(), // unicode eample
		// uncomment the long string for benchmarks
//		"Alan\nTuring\nwas\nan\nEnglish\nmathematician,\nlogician,\nand\ncomputer\nscientist.\nHe\nis\nwidely\nconsidered\none\nof\nthe\nfounders\nof\nthe\nfield\nof\ntheoretical\ncomputer\nscience\nand\nartificial\nintelligence.\nDuring\nWorld\nWar\nII,\nTuring\nplayed\na\nvital\nrole\nin\nbreaking\nthe\nGerman\nEnigma\ncode,\nwhich\nhelped\nthe\nAllied\nforces\nin\ntheir\nfight\nagainst\nthe\nNazis.\nTuring's\nwork\non\ncomputability\nand\nthe\nconcept\nof\na\nTuring\nmachine\nlaid\nthe\nfoundation\nfor\nmodern\ncomputer\nscience.\nDespite\nhis\ncontributions,\nTuring\nfaced\npersecution\nfor\nhis\nhomosexuality\nand\nwas\nconvicted\nof\n\"gross\nindecency.\"\nHe\npassed\naway\nin\n1954,\nbut\nhis\nlegacy\nand\ncontributions\nto\nthe\nfield\nof\ncomputing\ncontinue\nto\nbe\ncelebrated\nto\nthis\nday.".to_string(),	// long example
		"".to_string(),						 // empty file
	]; 

	let test_changes = ["", "x", "xy", "xyz", "\n", "\n\n", "\n\n\n", "\nx\n", "ć", "⁺", "𒀀"];

	//timers
	let mut lazy_timer = Duration::new(0, 0);
	let mut parse_timer = Duration::new(0, 0);

	// exhaustive search
	for file in test_files {
		let original_lsp = LspFile::new(&file.clone()); // Create default LspPos
		for change in test_changes {
			for size in 0..file.len() + 1 {
				for start_byte in 0..(file.len() - size + 1) {
					// generate Event
					let start = original_lsp.byte_to_lsp(start_byte);
					let end = original_lsp.byte_to_lsp(start_byte + size);

					let (l1, c1) = (start.line, start.character);
					let (l2, c2) = (end.line, end.character);
					let event = change_event((l1, c1), (l2, c2), change.to_string());

					let mut lsp = original_lsp.clone();

					let clock = Instant::now();
					lsp.lazy_add(&event);
					lazy_timer += clock.elapsed();

					// generate expected file
					let mut str = file.clone();
					let s = original_lsp.lsp_to_byte(&start);
					let e = original_lsp.lsp_to_byte(&end);
					
					let clock = Instant::now();
					str.replace_range(s..e, change);	// Would be required without lazy add
					let expected_lsp = LspFile::new(&str);
					parse_timer += clock.elapsed();

					if expected_lsp.get_ranges() != lsp.get_ranges() || str != *lsp.get_file() || expected_lsp.get_eof() !=  lsp.get_eof() {
						println!("Lazy time:    {}ns", lazy_timer.as_nanos());
						println!("Parser time:  {}ns", parse_timer.as_nanos());
						println!(
							"expected string: {:?}\nchange: {:?}\nsize: {}\nstart_pos: {:?}\nend_pos: {:?}\nstart_byte: {}",
							str, change, size, start, end, start_byte
						);
						assert_eq!(expected_lsp.get_ranges(), lsp.get_ranges());
						assert_eq!(str, *lsp.get_file());
						assert_eq!(expected_lsp.get_eof(), lsp.get_eof());
					}
				}
			}
		}
	}
	println!("Lazy time:    {}ns", lazy_timer.as_nanos());
	println!("Parser time:  {}ns", parse_timer.as_nanos());
}
