use markdown::mdast::{self, Node};
use std::path::PathBuf;

use crate::{markdown_file::{MdastDocument, File}, util::iterate_markdown_files};

fn archive_mdast(mdast: &mdast::Root) -> Option<mdast::Root> {
    // println!("mdast: {:#?}", markdown.body);

    let mut new_mdast: Vec<Node> = mdast.children.clone();

    // find or create the archived section
    let archived_section = new_mdast
        
        .iter()
        .enumerate()
        .find(|(_, node)| match node {
            Node::Heading(heading) => heading.depth == 2 && matches!(heading.children.first(), Some(Node::Text(text)) if text.value == "Archived"),
            _ => false,
        })
        .map(|(index, _)| index)
        .unwrap_or_else(|| {
            let archived_heading = mdast::Heading {
                depth: 2,
                children: vec![Node::Text(mdast::Text {
                    value: "Archived".to_string(),
                    position: None,
                })],
                position: None,
            };
            // find the last list
            let last_list = new_mdast
                
                .iter()
                .enumerate()
                .rev()
                .find(|(_, node)| matches!(node, Node::List(_)))
                .map(|(index, _)| index + 1)
                .unwrap_or_else(|| new_mdast.len());

            if last_list == new_mdast.len() {
                new_mdast.push(Node::Heading(archived_heading));
            } else {
                new_mdast.insert(last_list, Node::Heading(archived_heading));
            }

            last_list
        });

    fn should_archive(node: &Node) -> bool {
        match node {
            Node::ListItem(list_item) => match list_item.checked {
                Some(true) | None => list_item.children.iter().all(should_archive),
                Some(false) => false,
            },
            Node::List(list) => list.children.iter().all(should_archive),
            _ => true,
        }
    }

    let mut to_delete = vec![];
    for (i, node) in mdast
        .children
        .iter()
        .take(archived_section)
        .enumerate()
    {
        if let Node::List(list) = node {
            let archived_children: Vec<_> = list
                .children
                .iter()
                .enumerate()
                .filter_map(|(j, node)| match node {
                    Node::ListItem(list_item) if should_archive(node) => {
                        Some((j, Node::ListItem(list_item.clone())))
                    }
                    _ => None,
                })
                .collect();

            if archived_children.is_empty() {
                continue;
            }

            archived_children.iter().for_each(|(j, _)| {
                to_delete.push((i, *j));
            });

            let archived_list = mdast::List {
                children: archived_children
                    .into_iter()
                    .map(|(_, node)| node)
                    .collect(),
                ..list.clone()
            };

            if !archived_list.children.is_empty() {
                new_mdast.insert(archived_section + 1, Node::List(archived_list));
            }
        }
    }

    for (i, j) in to_delete.iter().rev() {
        if let Node::List(list) = &mut new_mdast[*i] {
            let mut new_children = list.children.clone();
            new_children.remove(*j);
            if new_children.is_empty() {
                new_mdast.remove(*i);
            } else {
                list.children = new_children;
            }
        } else {
            panic!("to_delete target should have been a list");
        }
    }

    if to_delete.is_empty() {
        return None;
    }

    Some(mdast::Root {
        children: new_mdast,
        position: None,
    })
}

pub fn archive(vault_path: PathBuf) {
    iterate_markdown_files(vault_path, "todo")
        .map(MdastDocument::parse)
        .filter_map(|document| match archive_mdast(&document.body) {
            Some(mdast) => Some(document.replace_with(document.frontmatter.clone(), mdast)),
            None => None,
        })
        .map(|f| f.atomic_overwrite())
        .for_each(Result::unwrap);
}

#[cfg(test)]
mod tests {
    use super::*;

    use indoc::indoc;
    use pretty_assertions::assert_eq;

    macro_rules! test_archive {
      ($($name:ident $input:expr => $expected:expr)*) => {
        $(
            #[test]
            fn $name() {
                let input = indoc!($input);
                let input_document = MdastDocument::parse(File::evil_fixme_from_string(input.to_string()));
                let expected = indoc!($expected);
                match archive_mdast(&input_document.body) {
                    Some(actual_mdast) => {
                        let actual = input_document.replace_with(input_document.frontmatter.clone(), actual_mdast).render();
                        assert_eq!(actual, expected);
                    },
                    None => assert!(input == expected, "archive_markdown returned None, but the expected output was not the input file. Input was:\n{}", input),
                }

            }
        )*
      }
    }

    test_archive! {

        untouched r#"
        - [ ] item 1
        "# => r#"
        - [ ] item 1
        "#

        archive_single_item r#"
        #todo
        - [x] item 1
        "# => r#"
        ## Archived

        - [x] item 1
        "#

        archive_multiple_items r#"
        - [x] item 1
        - [x] item 2
        - [ ] item 3
        "# => r#"
        - [ ] item 3

        ## Archived

        - [x] item 1
        - [x] item 2
        "#

        archive_multiple_items_with_sub_items r#"
        - [x] item 1
            - [x] item 1.1
            - [x] item 1.2
        - [x] item 2
            - [ ] item 2.1
        - [ ] item 3
        "# => r#"
        - [x] item 2
            - [ ] item 2.1
        - [ ] item 3

        ## Archived

        - [x] item 1
            - [x] item 1.1
            - [x] item 1.2
        "#
    }
}
