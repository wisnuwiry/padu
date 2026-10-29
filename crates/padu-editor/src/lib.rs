//! A Notion-style block editor for gpui, over the markdown document model.
//!
//! ```ignore
//! padu_editor::init(cx);
//! let editor = cx.new(|cx| padu_editor::Editor::new("# Title", cx));
//! ```

pub mod anchor;
pub mod editor;
pub mod history;
pub mod layout;
pub mod link;
pub mod model;
pub mod slash;
pub mod text_size;
pub mod theme;

pub use anchor::{Anchor, AnchorId, Delta};
pub use editor::{
    BlockPreviewMode, CONTEXT, Chrome, CustomBlockRenderer, Editor, EditorEvent, Formatting,
    HIGHLIGHT_MARK, Mode,
    image::{ImageStore, Source, set_image_store},
    init, keys,
    menu::{BLOCK_HANDLE, BLOCK_MENU, SLASH_MENU},
    turns,
};
pub use history::{DEFAULT_UNDO_LIMIT, EditKind, History, Step};
pub use layout::{Layout, set_layout};
pub use link::{Choice, Paste};
pub use model::{
    Align, Annotation, Block, BlockKind, Cursor, Doc, Form, HighlightColor, Mark, MarkSpan, Part,
    QuoteKind, Selection, Splice, Text, is_image, is_url, parse, serialize,
};
pub use slash::{Slash, items, label};
pub use text_size::{
    TextSize, adjust_text_size, reset_text_size, set_text_size, text_size_adjustment,
};
pub use theme::EditorTheme;

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{AppContext, TestAppContext};

    #[gpui::test]
    fn test_editor_creation_and_content(cx: &mut TestAppContext) {
        cx.update(|cx| {
            init(cx);
            let editor = cx.new(|cx| Editor::new("# Hello World\n\n- item 1\n- item 2", cx));
            let content = editor.read(cx).content();
            assert!(content.contains("# Hello World"));
            assert!(content.contains("- item 1"));
        });
    }

    #[gpui::test]
    fn test_editor_typing_and_undo(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let editor = cx.new(|cx| Editor::new("", cx));
            editor.update(cx, |ed, cx| {
                ed.insert_text("Initial text", cx);
            });
            assert!(editor.read(cx).content().contains("Initial text"));

            editor.update(cx, |ed, cx| {
                ed.undo(cx);
            });
            assert_eq!(editor.read(cx).content().trim(), "");
        });
    }

    #[gpui::test]
    fn test_editor_block_splitting(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let editor = cx.new(|cx| Editor::new("First block", cx));
            editor.update(cx, |ed, cx| {
                ed.split_block(cx);
            });
            assert_eq!(editor.read(cx).doc().blocks.len(), 2);
        });
    }

    #[gpui::test]
    fn test_anchor_mapping(cx: &mut TestAppContext) {
        cx.update(|_cx| {
            let start = Cursor::new(0, Part::Body, 0);
            let end = Cursor::new(0, Part::Body, 5);
            let mut anchor = Anchor::new(AnchorId(1), Selection::new(start, end));

            // Insertion after anchor
            let caret = Cursor::new(0, Part::Body, 10);
            let splice = Splice {
                removed: Selection::at(Cursor::new(0, Part::Body, 8)),
                caret,
                blocks: 0,
            };
            anchor.map(&Delta::Spliced(splice));
            assert_eq!(anchor.range.anchor.offset, 0);
            assert_eq!(anchor.range.head.offset, 5);
        });
    }

    #[gpui::test]
    fn test_slash_menu_filtering(cx: &mut TestAppContext) {
        cx.update(|_cx| {
            let mut slash = Slash::open(Cursor::default());
            slash.refilter("heading");
            assert!(!slash.rows.is_empty());
            let choice = slash.choice();
            assert!(matches!(choice, Some(BlockKind::Heading { .. })));
        });
    }

    #[gpui::test]
    fn test_link_paste_choices(cx: &mut TestAppContext) {
        cx.update(|_cx| {
            let paste = Paste::open(Cursor::default(), "https://example.com".to_string(), true);
            assert!(paste.rows.contains(&Choice::Bookmark));
            assert!(paste.rows.contains(&Choice::Chip));
        });
    }

    #[gpui::test]
    fn test_layout_and_text_size(cx: &mut TestAppContext) {
        cx.update(|cx| {
            set_layout(cx, Layout { text_inset: 30.0 });
            assert_eq!(Layout::of(cx).text_inset, 30.0);

            reset_text_size(cx);
            assert_eq!(text_size_adjustment(cx), 0.0);
            adjust_text_size(cx, 2.0);
            assert_eq!(text_size_adjustment(cx), 2.0);
        });
    }

    #[gpui::test]
    fn test_image_store(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let custom_store = ImageStore {
                keep: |_, _, _, _| Some("https://example.com/saved.png".to_string()),
                accepts: |_| true,
            };
            set_image_store(cx, custom_store);
            let store = ImageStore::of(cx);
            assert!((store.accepts)(std::path::Path::new("test.png")));
        });
    }

    #[gpui::test]
    fn test_block_operations(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let editor = cx.new(|cx| Editor::new("- [ ] Task item\n\nParagraph 2", cx));
            editor.update(cx, |ed, cx| {
                ed.toggle_task(0, cx);
                if let BlockKind::Task { checked, .. } = &ed.doc().blocks[0].kind {
                    assert!(*checked);
                } else {
                    panic!("expected task block");
                }

                ed.duplicate_block(0, cx);
                assert_eq!(ed.doc().blocks.len(), 3);

                ed.remove_block(1, cx);
                assert_eq!(ed.doc().blocks.len(), 2);
            });
        });
    }

    #[gpui::test]
    fn test_mode_switching(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let editor = cx.new(|cx| Editor::new("# Title", cx));
            editor.update(cx, |ed, cx| {
                assert_eq!(ed.mode(), Mode::Blocks);
                ed.set_mode(Mode::Source, cx);
                assert_eq!(ed.mode(), Mode::Source);
                ed.toggle_mode(cx);
                assert_eq!(ed.mode(), Mode::Blocks);
            });
        });
    }

    #[gpui::test]
    fn test_editor_clipboard_and_selection(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let editor = cx.new(|cx| Editor::new("Hello world markdown editor", cx));
            editor.update(cx, |ed, cx| {
                // Select all
                ed.select_all(cx);
                assert_eq!(
                    ed.selected_source(),
                    Some("Hello world markdown editor".to_string())
                );

                // Set collapsed selection
                ed.set_selection(Selection::at(Cursor::new(0, Part::Body, 5)), cx);
                assert_eq!(ed.selected_source(), None);

                // Paste plain text
                ed.paste_literal(" beautiful", cx);
                assert!(ed.content().contains("Hello beautiful world"));

                // Paste URL and confirm paste choice
                ed.paste_url("https://example.com/image.png".to_string(), cx);
                assert!(ed.paste_menu().is_some());
                ed.confirm_paste(Choice::Image, cx);
                assert!(ed.paste_menu().is_none());
            });
        });
    }

    #[gpui::test]
    fn test_editor_typing_mutations(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let editor = cx.new(|cx| Editor::new("First block\n\nSecond block", cx));
            editor.update(cx, |ed, cx| {
                // Indent and outdent
                assert_eq!(ed.doc().blocks[0].indent, 0);
                ed.indent(cx);
                assert_eq!(ed.doc().blocks[0].indent, 1);
                ed.outdent(cx);
                assert_eq!(ed.doc().blocks[0].indent, 0);

                // Delete backward character
                ed.set_selection(Selection::at(Cursor::new(0, Part::Body, 5)), cx);
                ed.delete_backward(cx); // deletes 't' in "First" -> "Firs"
                assert!(ed.content().contains("Firs block"));

                // Delete backward at start of block 1 merges into block 0
                ed.set_selection(Selection::at(Cursor::new(1, Part::Body, 0)), cx);
                ed.delete_backward(cx);
                assert_eq!(ed.doc().blocks.len(), 1);
                assert!(ed.content().contains("Second block"));

                // Delete forward
                ed.set_selection(Selection::at(Cursor::new(0, Part::Body, 0)), cx);
                ed.delete_forward(cx); // deletes 'F'
                assert!(!ed.content().starts_with('F'));

                // Toggle mark (bold)
                ed.set_selection(
                    Selection::new(Cursor::new(0, Part::Body, 0), Cursor::new(0, Part::Body, 3)),
                    cx,
                );
                ed.toggle_mark(Mark::Bold, cx);
                let formatting = ed.formatting();
                assert!(formatting.marks.contains(&Mark::Bold));

                // Untoggle mark
                ed.toggle_mark(Mark::Bold, cx);
                let formatting_after = ed.formatting();
                assert!(!formatting_after.marks.contains(&Mark::Bold));
            });
        });
    }

    #[gpui::test]
    fn test_editor_table_operations(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let md = "| A | B |\n| --- | --- |\n| 1 | 2 |";
            let editor = cx.new(|cx| Editor::new(md, cx));
            editor.update(cx, |ed, cx| {
                // Add row
                ed.add_row(0, 1, cx);
                if let BlockKind::Table { rows, .. } = &ed.doc().blocks[0].kind {
                    assert_eq!(rows.len(), 2);
                } else {
                    panic!("expected table");
                }

                // Add column
                ed.add_column(0, 2, cx);
                if let BlockKind::Table { header, rows, .. } = &ed.doc().blocks[0].kind {
                    assert_eq!(header.len(), 3);
                    assert_eq!(rows[0].len(), 3);
                }

                // Remove row
                ed.remove_row(0, 1, cx);
                if let BlockKind::Table { rows, .. } = &ed.doc().blocks[0].kind {
                    assert_eq!(rows.len(), 1);
                }

                // Remove column
                ed.remove_column(0, 2, cx);
                if let BlockKind::Table { header, .. } = &ed.doc().blocks[0].kind {
                    assert_eq!(header.len(), 2);
                }
            });
        });
    }

    #[gpui::test]
    fn test_editor_block_reordering(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let editor = cx.new(|cx| Editor::new("Block 1\n\nBlock 2\n\nBlock 3", cx));
            editor.update(cx, |ed, cx| {
                assert_eq!(
                    ed.doc().blocks[0].text_at(Part::Body).unwrap().text,
                    "Block 1"
                );
                assert_eq!(
                    ed.doc().blocks[1].text_at(Part::Body).unwrap().text,
                    "Block 2"
                );

                // Move Block 2 up
                ed.move_block_up(1, cx);
                assert_eq!(
                    ed.doc().blocks[0].text_at(Part::Body).unwrap().text,
                    "Block 2"
                );
                assert_eq!(
                    ed.doc().blocks[1].text_at(Part::Body).unwrap().text,
                    "Block 1"
                );

                // Move Block 2 down
                ed.move_block_down(0, cx);
                assert_eq!(
                    ed.doc().blocks[0].text_at(Part::Body).unwrap().text,
                    "Block 1"
                );
                assert_eq!(
                    ed.doc().blocks[1].text_at(Part::Body).unwrap().text,
                    "Block 2"
                );
            });
        });
    }

    #[gpui::test]
    fn test_editor_anchors_tracking(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let editor = cx.new(|cx| Editor::new("Some text here", cx));
            editor.update(cx, |ed, cx| {
                let anchor = Anchor::new(
                    AnchorId(42),
                    Selection::new(Cursor::new(0, Part::Body, 5), Cursor::new(0, Part::Body, 9)),
                );
                ed.add_anchor(anchor);
                assert_eq!(ed.anchors().len(), 1);

                // Hit testing
                let found = ed.anchor_at(Cursor::new(0, Part::Body, 7));
                assert!(found.is_some());
                assert_eq!(found.unwrap().id, AnchorId(42));

                let not_found = ed.anchor_at(Cursor::new(0, Part::Body, 2));
                assert!(not_found.is_none());

                // Activation event
                ed.activate_anchor(AnchorId(42), cx);

                // Remove anchor
                ed.remove_anchor(AnchorId(42));
                assert_eq!(ed.anchors().len(), 0);
            });
        });
    }

    #[gpui::test]
    fn test_editor_caret_and_pointer(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let editor = cx.new(|cx| Editor::new("Line 1\n\nLine 2", cx));
            editor.update(cx, |ed, cx| {
                ed.set_caret_visible(false, cx);
                ed.caret_moved();
                assert_eq!(ed.cursor(), Cursor::new(0, Part::Body, 0));

                // Mouse down at block 1 offset 3
                ed.on_mouse_down(1, Part::Body, 3, cx);
                assert_eq!(ed.cursor(), Cursor::new(1, Part::Body, 3));

                // Mouse drag to block 1 offset 5
                ed.on_mouse_drag(1, Part::Body, 5, cx);
                assert_eq!(ed.selection().anchor, Cursor::new(1, Part::Body, 3));
                assert_eq!(ed.selection().head, Cursor::new(1, Part::Body, 5));

                // Dismiss popovers
                ed.dismiss_popovers(cx);
            });
        });
    }

    #[gpui::test]
    fn test_editor_entity_input_handler(cx: &mut TestAppContext) {
        use gpui::EntityInputHandler;

        let (editor, cx) =
            cx.add_window_view(|_window, cx| Editor::new("Text for input testing", cx));

        cx.update(|window, cx| {
            editor.update(cx, |ed, cx| {
                let mut adjusted = None;
                let text = ed.text_for_range(0..4, &mut adjusted, window, cx);
                assert_eq!(text, Some("Text".to_string()));
                assert_eq!(adjusted, Some(0..4));

                let sel_range = ed.selected_text_range(false, window, cx);
                assert!(sel_range.is_some());

                // Replace text in range
                ed.replace_text_in_range(Some(0..4), "Word", window, cx);
                assert!(ed.content().starts_with("Word"));

                // Replace and mark text (IME composition)
                ed.replace_and_mark_text_in_range(Some(0..4), "Compose", Some(0..7), window, cx);
                assert_eq!(ed.marked_text_range(window, cx), Some(0..7));

                // Unmark
                ed.unmark_text(window, cx);
                assert_eq!(ed.marked_text_range(window, cx), None);
            });
        });
    }

    #[gpui::test]
    fn test_mermaid_and_latex_slash_items(cx: &mut TestAppContext) {
        cx.update(|_cx| {
            let mut slash = Slash::open(Cursor::default());
            slash.refilter("mermaid");
            assert!(!slash.rows.is_empty());
            let choice = slash.choice();
            assert!(matches!(
                choice,
                Some(BlockKind::Code {
                    language: Some(ref l),
                    ..
                }) if l == "mermaid"
            ));

            slash.refilter("latex");
            assert!(!slash.rows.is_empty());
            let choice = slash.choice();
            assert!(matches!(
                choice,
                Some(BlockKind::Code {
                    language: Some(ref l),
                    ..
                }) if l == "latex"
            ));
        });
    }

    #[gpui::test]
    fn test_code_block_preview_mode_and_custom_renderer(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let editor = cx.new(|cx| {
                Editor::new(
                    "```mermaid\ngraph TD\n  A-->B\n```\n\n```rust\nfn main() {}\n```",
                    cx,
                )
            });

            editor.update(cx, |ed, cx| {
                // Block 0 is mermaid, should default to Preview
                assert_eq!(ed.block_preview_mode(0), BlockPreviewMode::Preview);
                // Block 1 is rust, should default to Code
                assert_eq!(ed.block_preview_mode(1), BlockPreviewMode::Code);

                // Switch block 0 to Split
                ed.set_block_preview_mode(0, BlockPreviewMode::Split, cx);
                assert_eq!(ed.block_preview_mode(0), BlockPreviewMode::Split);

                // Theme configuration
                let custom_theme = EditorTheme {
                    is_dark: true,
                    ..Default::default()
                };
                ed.set_theme(custom_theme, cx);
                assert_eq!(ed.theme().is_dark, true);

                assert!(!ed.has_custom_preview_renderer());
                ed.set_custom_preview_renderer(Some(std::rc::Rc::new(|_, _, _, _, _| None)), cx);
                assert!(ed.has_custom_preview_renderer());
            });
        });
    }

    #[gpui::test]
    fn test_editor_focus_end(cx: &mut TestAppContext) {
        let (editor, cx) = cx.add_window_view(|_window, cx| {
            Editor::new("First paragraph\n\n```rust\nfn main() {}\n```", cx)
        });

        cx.update(|window, cx| {
            editor.update(cx, |ed, cx| {
                assert!(!ed.has_custom_preview_renderer());
                // Calling focus_end when last block is Code should append a paragraph for Notion-like behavior
                ed.focus_end(window, cx);
                assert_eq!(ed.doc().blocks.len(), 3);
                assert_eq!(ed.cursor(), Cursor::new(2, Part::Body, 0));

                // If document ends with text, focus_end puts cursor at the end
                ed.insert_text("Trailing text", cx);
                ed.focus_end(window, cx);
                assert_eq!(ed.cursor(), Cursor::new(2, Part::Body, 13));
            });
        });
    }

    #[gpui::test]
    fn test_code_multiline_editing_and_navigation(cx: &mut TestAppContext) {
        cx.update(|cx| {
            let editor = cx.new(|cx| Editor::new("```rust\nlet a = 1;\nlet b = 2;\n```", cx));

            editor.update(cx, |ed, cx| {
                // Put caret at offset 10 in Part::Code
                ed.set_selection(Selection::at(Cursor::new(0, Part::Code, 10)), cx);
                // Pressing Enter in Part::Code should insert newline, not split into paragraph
                ed.split_block(cx);
                assert_eq!(ed.doc().blocks.len(), 1);
                assert!(ed.content().contains("let a = 1;\n\nlet b = 2;"));

                // Reset content for clean vertical line navigation test
                ed.set_content("```rust\nlet a = 1;\nlet b = 2;\n```", cx);
                ed.set_selection(Selection::at(Cursor::new(0, Part::Code, 4)), cx); // on line 1 ("let a = 1;")
                ed.move_down(cx);
                assert_eq!(ed.selection().head.offset, 15); // moved to line 2 col 4 ("let b = 2;")
                ed.move_up(cx);
                assert_eq!(ed.selection().head.offset, 4); // moved back up to line 1 col 4
            });
        });
    }
}
