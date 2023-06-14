use std::{path::PathBuf, fs::File, io::Write, sync::Arc, collections::HashMap};

use lsp_types::{lsif::*, NumberOrString, Hover, HoverContents, MarkedString, Url};

use crate::flags::LsifP4Cmd;

// TODO: Make it a PImpl design in Rust - stop anyone from accessing id or file accidentaly
/// LsifWriter is a simple API that handles file creation to the LSIF standard
/// Aim is for each 'append' function to be given the required data it needs and produce the Vertex & add it to the file.
/// It should also link it with the required Edges.
/// Then return the newly created Vertex ID for producer to reference for other Edges later on
pub struct LsifWriter {
    settings: Arc<LsifP4Cmd>,
    id: i32,        // never call this directly, use get_id()
    file: String,   // never call this directly, use append_file()
    data: LsifData, // Data could belong to the writer or generator but here it makes sure the data and file matches up
}

pub struct LsifData {
    files: HashMap<Url, i32>,
}

impl LsifData {
    pub fn new() -> Self {  LsifData { files: HashMap::new() }  }
}

impl LsifWriter {
    pub fn new(settings: Arc<LsifP4Cmd>) -> Self {
        LsifWriter{settings, id: 0, file: String::new(), data: LsifData::new() }
    }

    // Increments automatically as you should never reuse an ID number 
    fn get_id(&mut self) -> i32 {
        let ret = self.id;
        self.id += 1;
        ret
    }

    pub fn i32_to_enum(i:i32) -> NumberOrString {
        NumberOrString::Number(i)
    }

    pub fn data(&self) -> &LsifData { 
        &self.data
    }

    // This makes sure everything written to the file is of type Entry and in JSon format
    pub fn append_file(&mut self, entry: &Entry) {
        let mut json = serde_json::to_string(entry).expect("Failed to serialze entry");
        json.push('\n');
        self.file.push_str(&json);
    }

    pub fn write_file_to_disk(&self) {
        // This code deals with if CLI arguments are given to or not
        let dest = self.settings.output.clone().unwrap_or_else(|| PathBuf::from("."));
        let filename = self.settings.filename.clone().unwrap_or_else(|| "P4Analysis".to_string());
        let filepath = dest.join(format!("{}.lsif", filename));
    
        let mut file = File::create(&filepath).expect("Failed to create output file!");
        file.write_all(self.file.as_bytes()).expect("Failed to write to output file!");
        println!("Finished Generating LSIF file to {:?}", filepath);
    }

    pub fn document(&mut self, url: Url) -> i32 {
        let new_id = self.get_id();
        let id = Self::i32_to_enum(new_id);

        let vertex = Entry { id,
            data: Element::Vertex(Vertex::Document(Document { uri: url.clone(), language_id: "P4".into() }))
        };
        
        self.append_file(&vertex);
        self.data.files.insert(url, new_id);

        new_id
    }

    // https://microsoft.github.io/language-server-protocol/specifications/lsif/0.4.0/specification/#textDocument_hover
    pub fn text_document_hover(&mut self) -> i32 {
        // get next id number but store as variable as have to reference it in the edge
        let new_id = self.get_id();
        let id = Self::i32_to_enum(new_id);
        // Everything that gets added to the file has to be on type Entry
        // We generate the Vertex first as Edges references them
        // What seperate the 2 types is the Element Enum
        // How to generate different Vertex types is with the Vertex Enum (and fill it accordingly)
        let vertex = Entry{ id, 
            data: Element::Vertex(Vertex::HoverResult{ result: Hover{ contents: HoverContents::Scalar(MarkedString::String("hi".to_string())), range: todo!() } })};

        // Edges are much simpilar but there can be many more and it's more difficult to link them correctly
        let edge = Entry{ id,
            data: Element::Edge(Edge::Hover(EdgeData{ in_v: todo!(), out_v: id })) };

        // Add it to file ready for writing
        self.append_file(&vertex);
        self.append_file(&edge);

        new_id
    }
}
