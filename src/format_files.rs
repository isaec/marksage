use std::{borrow::Cow, path::PathBuf};

use lazy_static::lazy_static;
use markdown::mdast::{self, Node};
use rayon::prelude::ParallelIterator;
use regex::Regex;
use replace_with::replace_with_or_abort;

use crate::{markdown_file::MdastDocument, util::iterate_markdown_files};

lazy_static! {
    static ref EM_DASH_REPLACE: Regex = Regex::new("([[:alnum:]])(--)([[:alnum:]])").unwrap();
}

fn text_replace(text: String) -> String {
    match EM_DASH_REPLACE.replace_all(&text, "${1}—${3}") {
        Cow::Borrowed(_) => text,
        Cow::Owned(text) => text,
    }
}

fn format_node(mut node: Node) -> Node {
    match node {
        Node::Text(text) => Node::Text(mdast::Text {
            value: text_replace(text.value),
            position: None, // position may be changed by text replacement
        }),
        _ => {
            if let Some(children) = node.children_mut() {
                for child in children.iter_mut() {
                    replace_with_or_abort(child, format_node);
                }
            }
            node
        }
    }
}

fn format_document(document: MdastDocument) -> MdastDocument {
    let new_body = match format_node(Node::Root(document.body)) {
        Node::Root(root) => root,
        _ => unreachable!(),
    };

    MdastDocument {
        body: new_body,
        frontmatter: document.frontmatter,
    }
}

#[must_use]
pub fn format_files(vault_path: &PathBuf) -> impl ParallelIterator<Item = (PathBuf, String)> {
    iterate_markdown_files(vault_path)
        .map(|file| (file.path, MdastDocument::parse(file.content.as_str())))
        .map(|(path, document)| (path, format_document(document).render()))
}
