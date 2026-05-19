use super::SourceTree;
use achitekfile::TextRange;
use tree_sitter::Node;

pub(super) fn prompt_type_for_block<'a>(
    syntax: &'a SourceTree,
    prompt_block: Node<'_>,
) -> Option<&'a str> {
    for index in 0..prompt_block.child_count() {
        let Some(child) =
            prompt_block.child(u32::try_from(index).expect("child index should fit into u32"))
        else {
            continue;
        };

        if child.kind() == "question_attribute" {
            for nested_index in 0..child.child_count() {
                let Some(nested) = child
                    .child(u32::try_from(nested_index).expect("child index should fit into u32"))
                else {
                    continue;
                };

                if nested.kind() == "type_attribute" {
                    let value = nested.child_by_field_name("value")?;
                    return Some(syntax.text_for(value));
                }
            }
        }
    }

    None
}

pub(super) fn prompt_block_for_range<'a>(
    syntax: &'a SourceTree,
    range: TextRange,
) -> Option<Node<'a>> {
    let root = syntax.root_node();

    for index in 0..root.child_count() {
        let Some(child) =
            root.child(u32::try_from(index).expect("child index should fit into u32"))
        else {
            continue;
        };

        if child.kind() == "prompt_block" && syntax.range_for(child) == range {
            return Some(child);
        }
    }

    None
}
