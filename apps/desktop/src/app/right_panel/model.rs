use super::*;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum FileOperationDialogKind {
    CreateFile { parent: PathBuf },
    CreateDirectory { parent: PathBuf },
    Rename { source: PathBuf },
}

pub(crate) struct FileOperationDialog {
    pub(crate) kind: FileOperationDialogKind,
    pub(crate) input: Entity<TextInput>,
    pub(crate) focus: FocusHandle,
    pub(crate) previous_focus: Option<FocusHandle>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct WorkingTreeEntry {
    pub(crate) relative_path: String,
    pub(crate) absolute_path: PathBuf,
    pub(crate) name: String,
    pub(crate) is_dir: bool,
    pub(crate) is_ignored: bool,
    pub(crate) file_icon: Option<&'static str>,
    pub(crate) expanded: bool,
    pub(crate) depth: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum ReviewDiffTreeRow {
    Directory {
        path: String,
        name: String,
        depth: usize,
        expanded: bool,
    },
    File {
        file_index: usize,
        depth: usize,
    },
}

/// Select from a compact, embedded subset of Material Icon Theme rather than
/// shipping its entire icon catalog. The SVG path is resolved once per entry
/// during the directory scan, not on every row paint.
pub(crate) fn file_icon_for_path(path: &str) -> &'static str {
    let name = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path);
    file_icon_for_name(name)
}

pub(crate) fn review_diff_gap_icon_path(
    direction: crate::review_diff::ExpansionDirection,
) -> &'static str {
    match direction {
        // Pierre's direction attributes and rendered chevrons are inverted by
        // CSS. Padu names the data operation directly, so encode the resulting
        // visual here: reveal-from-start points down; reveal-from-end points up.
        crate::review_diff::ExpansionDirection::Start => "icons/chevron-down.svg",
        crate::review_diff::ExpansionDirection::End => "icons/chevron-up.svg",
        crate::review_diff::ExpansionDirection::Both
        | crate::review_diff::ExpansionDirection::All => "icons/chevrons-up-down.svg",
    }
}

pub(crate) fn review_diff_gap_tooltip(direction: crate::review_diff::ExpansionDirection) -> String {
    match direction {
        crate::review_diff::ExpansionDirection::Start => tr!("diff.expand_context_below"),
        crate::review_diff::ExpansionDirection::End => tr!("diff.expand_context_above"),
        crate::review_diff::ExpansionDirection::Both => tr!("diff.expand_context"),
        crate::review_diff::ExpansionDirection::All => tr!("diff.expand_all_context"),
    }
}

pub(crate) fn review_diff_gap_directions(
    position: crate::review_diff::GapPosition,
    chunked: bool,
) -> &'static [crate::review_diff::ExpansionDirection] {
    use crate::review_diff::{ExpansionDirection, GapPosition};

    match (position, chunked) {
        (GapPosition::Leading, _) => &[ExpansionDirection::End],
        (GapPosition::Trailing, _) => &[ExpansionDirection::Start],
        (GapPosition::Between, false) => &[ExpansionDirection::Both],
        (GapPosition::Between, true) => &[ExpansionDirection::Start, ExpansionDirection::End],
    }
}

pub(crate) fn review_diff_directory_paths(files: &[crate::review_diff::File]) -> HashSet<String> {
    let mut paths = HashSet::new();
    for file in files {
        let parts = file.path.split('/').collect::<Vec<_>>();
        let mut path = String::new();
        for part in parts.iter().take(parts.len().saturating_sub(1)) {
            if !path.is_empty() {
                path.push('/');
            }
            path.push_str(part);
            paths.insert(path.clone());
        }
    }
    paths
}

pub(crate) fn review_diff_tree_rows(
    files: &[crate::review_diff::File],
    expanded_paths: &HashSet<String>,
    filter: &str,
) -> Vec<ReviewDiffTreeRow> {
    let filter = filter.trim().to_ascii_lowercase();
    let filtering = !filter.is_empty();
    let mut indexes = files
        .iter()
        .enumerate()
        .filter(|(_, file)| {
            filtering
                .then(|| file.path.to_ascii_lowercase().contains(&filter))
                .unwrap_or(true)
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    indexes.sort_by_key(|index| files[*index].path.to_ascii_lowercase());

    let mut rows = Vec::new();
    let mut emitted_directories = HashSet::new();
    for file_index in indexes {
        let parts = files[file_index].path.split('/').collect::<Vec<_>>();
        let mut directory = String::new();
        let mut visible = true;
        for (depth, part) in parts.iter().take(parts.len().saturating_sub(1)).enumerate() {
            if !directory.is_empty() {
                directory.push('/');
            }
            directory.push_str(part);
            let expanded = filtering || expanded_paths.contains(&directory);
            if emitted_directories.insert(directory.clone()) && visible {
                rows.push(ReviewDiffTreeRow::Directory {
                    path: directory.clone(),
                    name: (*part).to_owned(),
                    depth,
                    expanded,
                });
            }
            if !expanded {
                visible = false;
                break;
            }
        }
        if visible {
            rows.push(ReviewDiffTreeRow::File {
                file_index,
                depth: parts.len().saturating_sub(1),
            });
        }
    }
    rows
}

/// How wide and tall a diff row is drawn. The Review panel is a reading
/// surface; the copy embedded in a transcript activity is a summary and gives
/// its space back to the code.
#[derive(Clone, Copy)]
pub(crate) struct DiffRowStyle {
    pub(crate) gutter_width: f32,
    pub(crate) row_height: f32,
    pub(crate) text_size: f32,
    /// What to put in the gutter of a row that has no line number. Git always
    /// reports positions, so this only comes up on a diff synthesized from a
    /// provider's before/after text: there the `+`/`-` marker stands in, which
    /// keeps the gutter from going blank and the meaning off color alone.
    pub(crate) marker_fallback: bool,
}

impl DiffRowStyle {
    /// Review-tab rows at the user's code font size. The gutter holds a
    /// right-aligned line number: ~0.6em per mono digit, five digits, plus
    /// its padding and border.
    pub(crate) fn review(text_size: f32) -> Self {
        Self {
            gutter_width: (text_size * 3.0 + 14.0).round(),
            row_height: (text_size * 1.5).round(),
            text_size,
            marker_fallback: false,
        }
    }

    /// The same rows the Review tab draws, so an edit reads the same wherever
    /// it is opened.
    pub(crate) fn activity(text_size: f32) -> Self {
        Self {
            marker_fallback: true,
            ..Self::review(text_size)
        }
    }

    pub(crate) fn gutter_width(&self) -> f32 {
        self.gutter_width
    }
}

/// Selection identity for one diff code row. Selection resolves a drag by
/// looking rows up by key, so every row must have its own.
///
/// Rows with line numbers key on them: they survive Review's gap expansion,
/// where a revealed gap shifts every later row's index. Rows without them — a
/// diff synthesized from a provider's before/after text — key on the row index
/// instead, which is stable there because an activity diff is only ever
/// rebuilt whole. Keying those on their (absent) numbers gave every added row
/// the same key, and a drag resolved against whichever duplicate registered
/// first: selections jumped rows, skipped wrapped lines, and collapsed when
/// the head crossed into context.
pub(crate) fn diff_row_selection_key(
    key_prefix: &str,
    line: &crate::review_diff::Line,
    index: usize,
) -> String {
    let kind = match &line.kind {
        crate::review_diff::LineKind::Context => "context",
        crate::review_diff::LineKind::Addition => "addition",
        crate::review_diff::LineKind::Deletion => "deletion",
        _ => "other",
    };
    match (line.old_line, line.new_line) {
        (None, None) => format!("{key_prefix}-line-{}-{kind}-i{index}", line.file_index),
        (old, new) => format!(
            "{key_prefix}-line-{}-{kind}-{}-{}",
            line.file_index,
            old.unwrap_or(0),
            new.unwrap_or(0),
        ),
    }
}

/// One context, addition, or deletion row, shared by the Review panel and the
/// diff inside an expanded file-change activity so the two never drift.
pub(crate) fn render_diff_code_row(
    line: &crate::review_diff::Line,
    index: usize,
    key_prefix: &str,
    selection: &TranscriptSelection,
    style: DiffRowStyle,
    theme: &Theme,
) -> AnyElement {
    let semantic_body_opacity = if theme.is_dark { 0.20 } else { 0.12 };
    let semantic_gutter_opacity = if theme.is_dark { 0.15 } else { 0.09 };
    let (marker, body_background, gutter_background, edge, number_color) = match &line.kind {
        crate::review_diff::LineKind::Addition => (
            "+",
            Some(theme.success.opacity(semantic_body_opacity)),
            Some(theme.success.opacity(semantic_gutter_opacity)),
            Some(theme.success),
            theme.success,
        ),
        crate::review_diff::LineKind::Deletion => (
            "-",
            Some(theme.danger.opacity(semantic_body_opacity)),
            Some(theme.danger.opacity(semantic_gutter_opacity)),
            Some(theme.danger),
            theme.danger,
        ),
        _ => (" ", None, None, None, theme.text_tertiary),
    };
    let shown_line = line.new_line.or(line.old_line);
    let flat = review_diff_flat_text(line, theme);
    let selectable = md::render::selectable_flat_text(
        &flat,
        crate::md::selection::TextKey::new(diff_row_selection_key(key_prefix, line, index), 0),
        selection.clone(),
        theme.code_wash,
        theme.selection,
        false,
    );
    let gutter = div()
        .w(px(style.gutter_width))
        .min_h(px(style.row_height))
        .self_stretch()
        .flex_none()
        .pr(px(9.0))
        .flex()
        .items_start()
        .justify_end()
        .border_r_1()
        .border_color(theme.border)
        .text_color(number_color)
        .when_some(gutter_background, |gutter, background| {
            gutter.bg(background)
        })
        .child(
            shown_line
                .map(|line| line.to_string())
                .or_else(|| style.marker_fallback.then(|| marker.to_owned()))
                .unwrap_or_default(),
        );
    let body = div()
        .min_h(px(style.row_height))
        .self_stretch()
        .min_w_0()
        .flex_1()
        .pl(px(12.0))
        .flex()
        .items_start()
        .when_some(body_background, |body, background| body.bg(background))
        .child(
            div()
                .id(SharedString::from(format!(
                    "{key_prefix}-line-content-{index}"
                )))
                .min_h(px(style.row_height))
                .min_w_0()
                .flex_1()
                .pr(px(10.0))
                .flex()
                .items_start()
                .overflow_hidden()
                .whitespace_normal()
                .child(selectable),
        );
    div()
        .id(SharedString::from(format!("{key_prefix}-row-{index}")))
        .w_full()
        .min_w_0()
        .min_h(px(style.row_height))
        // A wrapped line makes the row taller than one line. Stacked in a
        // scrolling column, a shrinkable row would be squeezed back to one
        // and paint its overflow over the row beneath it.
        .flex_none()
        .flex()
        .items_stretch()
        .font_family(md::render::MONO_FAMILY)
        .text_size(px(style.text_size))
        .line_height(px(style.row_height))
        .when_some(edge, |row, edge| row.border_l_2().border_color(edge))
        .child(gutter)
        .child(body)
        .into_any_element()
}

pub(crate) fn review_diff_flat_text(
    line: &crate::review_diff::Line,
    theme: &Theme,
) -> md::render::FlatText {
    let text = line.content.clone();
    let palette = MarkdownPalette::from_theme(theme);
    let code_font = font(md::render::MONO_FAMILY);
    let mut runs = Vec::with_capacity(line.tokens.len() * 2 + 1);
    let mut offset = 0;
    let mut push = |len: usize, color: Hsla| {
        if len > 0 {
            runs.push(TextRun {
                len,
                font: code_font.clone(),
                color,
                background_color: None,
                underline: None,
                strikethrough: None,
            });
        }
    };
    for token in &line.tokens {
        if token.range.start > offset {
            push(token.range.start - offset, theme.text_secondary);
        }
        push(token.range.len(), palette.token(token.class));
        offset = token.range.end;
    }
    if offset < text.len() {
        push(text.len() - offset, theme.text_secondary);
    }
    md::render::FlatText {
        text: text.into(),
        runs,
        links: Vec::new(),
        code_ranges: Vec::new(),
    }
}

pub(crate) fn file_icon_for_name(name: &str) -> &'static str {
    let name = name.to_ascii_lowercase();
    let named_icon = if name.starts_with("readme") {
        Some("icons/file-types/readme.svg")
    } else if name.starts_with("license")
        || name.starts_with("licence")
        || name.starts_with("copying")
    {
        Some("icons/file-types/certificate.svg")
    } else if name.starts_with("dockerfile") || name.starts_with("compose.") {
        Some("icons/file-types/docker.svg")
    } else if name == "cmakelists.txt" || name.starts_with("cmake.") {
        Some("icons/file-types/cmake.svg")
    } else if name == "makefile" || name.starts_with("makefile.") || name == "justfile" {
        Some("icons/file-types/makefile.svg")
    } else if matches!(
        name.as_str(),
        "cargo.toml" | "cargo.lock" | "rust-toolchain.toml"
    ) {
        Some("icons/file-types/rust.svg")
    } else if matches!(name.as_str(), "go.mod" | "go.sum" | "go.work") {
        Some("icons/file-types/go.svg")
    } else if name == "pyproject.toml" || name == "pipfile" || name.starts_with("requirements") {
        Some("icons/file-types/python.svg")
    } else if matches!(name.as_str(), "bun.lock" | "bun.lockb" | "bunfig.toml") {
        Some("icons/file-types/bun.svg")
    } else if name.starts_with("pnpm-") || name == ".pnpmfile.cjs" {
        Some("icons/file-types/pnpm.svg")
    } else if name == "yarn.lock" || name.starts_with(".yarnrc") {
        Some("icons/file-types/yarn.svg")
    } else if name == "package.json" {
        Some("icons/file-types/nodejs.svg")
    } else if name == "package-lock.json" {
        Some("icons/file-types/npm.svg")
    } else if name.starts_with("tsconfig.") || name == "tsconfig.json" {
        Some("icons/file-types/typescript.svg")
    } else if name.starts_with("jsconfig.") || name == "jsconfig.json" {
        Some("icons/file-types/javascript.svg")
    } else if name == ".gitignore"
        || name == ".gitattributes"
        || name == ".gitmodules"
        || name == ".gitconfig"
    {
        Some("icons/file-types/git.svg")
    } else if name == ".editorconfig" {
        Some("icons/file-types/editorconfig.svg")
    } else if name.starts_with(".env") {
        Some("icons/file-types/settings.svg")
    } else if name.starts_with(".prettier") || name.starts_with("prettier.config.") {
        Some("icons/file-types/prettier.svg")
    } else if name.starts_with(".eslint") || name.starts_with("eslint.config.") {
        Some("icons/file-types/eslint.svg")
    } else if name.starts_with("biome.json") {
        Some("icons/file-types/biome.svg")
    } else if name.starts_with(".babel") || name.starts_with("babel.config.") {
        Some("icons/file-types/babel.svg")
    } else if name.starts_with(".stylelint") || name.starts_with("stylelint.config.") {
        Some("icons/file-types/stylelint.svg")
    } else if name.starts_with("vite.config.") {
        Some("icons/file-types/vite.svg")
    } else if name.starts_with("vitest.config.") || name.starts_with("vitest.workspace.") {
        Some("icons/file-types/vitest.svg")
    } else if name.starts_with("webpack.") {
        Some("icons/file-types/webpack.svg")
    } else if name.starts_with("rollup.config.") {
        Some("icons/file-types/rollup.svg")
    } else if name.starts_with("next.config.") {
        Some("icons/file-types/next.svg")
    } else if name == "next-env.d.ts" {
        Some("icons/file-types/next.svg")
    } else if name.starts_with("nuxt.config.") || name == ".nuxtrc" {
        Some("icons/file-types/nuxt.svg")
    } else if name.starts_with("astro.config.") {
        Some("icons/file-types/astro.svg")
    } else if name == "angular.json" || name.ends_with(".component.ts") {
        Some("icons/file-types/angular.svg")
    } else if name == "nest-cli.json" {
        Some("icons/file-types/nest.svg")
    } else if name.starts_with("tailwind.config.") {
        Some("icons/file-types/tailwindcss.svg")
    } else if name.starts_with("svelte.config.") {
        Some("icons/file-types/svelte.svg")
    } else if name.starts_with("vue.config.") {
        Some("icons/file-types/vue.svg")
    } else if name == "firebase.json" || name == ".firebaserc" {
        Some("icons/file-types/firebase.svg")
    } else if name == "supabase.toml" {
        Some("icons/file-types/supabase.svg")
    } else if name.starts_with("prisma.config.") {
        Some("icons/file-types/prisma.svg")
    } else if name == "turbo.json" {
        Some("icons/file-types/turborepo.svg")
    } else if name.starts_with("deno.json") || name == "deno.lock" {
        Some("icons/file-types/deno.svg")
    } else if name == ".gitlab-ci.yml" || name == ".gitlab-ci.yaml" {
        Some("icons/file-types/gitlab.svg")
    } else if name == "kustomization.yaml" || name == "kustomization.yml" {
        Some("icons/file-types/kubernetes.svg")
    } else if name == "chart.yaml" || name == "values.yaml" {
        Some("icons/file-types/helm.svg")
    } else if name == "nginx.conf" {
        Some("icons/file-types/nginx.svg")
    } else if name == ".nvmrc" || name == ".node-version" {
        Some("icons/file-types/nodejs.svg")
    } else if name == "build.gradle"
        || name == "settings.gradle"
        || name == "gradlew"
        || name == "gradlew.bat"
    {
        Some("icons/file-types/gradle.svg")
    } else if name.contains(".stories.") || name.contains(".story.") {
        Some("icons/file-types/storybook.svg")
    } else if name == "gemfile" || name == "gemfile.lock" {
        Some("icons/file-types/ruby.svg")
    } else if name == "pom.xml" {
        Some("icons/file-types/java.svg")
    } else {
        None
    };
    if let Some(icon) = named_icon {
        return icon;
    }

    let extension = Path::new(&name)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or("");
    match extension {
        "rs" => "icons/file-types/rust.svg",
        "js" | "mjs" | "cjs" => "icons/file-types/javascript.svg",
        "ts" | "mts" | "cts" => "icons/file-types/typescript.svg",
        "jsx" | "tsx" => "icons/file-types/react.svg",
        "py" | "pyi" | "pyw" => "icons/file-types/python.svg",
        "go" => "icons/file-types/go.svg",
        "c" | "h" | "m" => "icons/file-types/c.svg",
        "cc" | "cpp" | "cxx" | "hh" | "hpp" | "hxx" | "mm" => "icons/file-types/cpp.svg",
        "cs" => "icons/file-types/csharp.svg",
        "swift" => "icons/file-types/swift.svg",
        "kt" | "kts" => "icons/file-types/kotlin.svg",
        "java" | "class" => "icons/file-types/java.svg",
        "rb" => "icons/file-types/ruby.svg",
        "php" => "icons/file-types/php.svg",
        "html" | "htm" => "icons/file-types/html.svg",
        "css" | "less" => "icons/file-types/css.svg",
        "scss" | "sass" => "icons/file-types/sass.svg",
        "json" | "jsonc" | "jsonl" => "icons/file-types/json.svg",
        "yaml" | "yml" => "icons/file-types/yaml.svg",
        "toml" | "ini" | "cfg" | "conf" | "config" => "icons/file-types/settings.svg",
        "xml" | "xsl" | "plist" => "icons/file-types/xml.svg",
        "md" | "mdx" | "markdown" => "icons/file-types/markdown.svg",
        "sh" | "bash" | "zsh" | "fish" => "icons/file-types/console.svg",
        "ps1" | "psm1" => "icons/file-types/powershell.svg",
        "sql" | "db" | "sqlite" | "sqlite3" | "csv" | "xls" | "xlsx" => {
            "icons/file-types/database.svg"
        }
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "avif" | "ico" | "tiff" => {
            "icons/file-types/image.svg"
        }
        "svg" => "icons/file-types/svg.svg",
        "pdf" => "icons/file-types/pdf.svg",
        "mp3" | "wav" | "flac" | "ogg" | "m4a" => "icons/file-types/audio.svg",
        "mp4" | "mov" | "avi" | "webm" | "mkv" => "icons/file-types/video.svg",
        "zip" | "gz" | "tgz" | "bz2" | "xz" | "7z" | "rar" | "tar" | "jar" => {
            "icons/file-types/zip.svg"
        }
        "wasm" | "wat" => "icons/file-types/webassembly.svg",
        "svelte" => "icons/file-types/svelte.svg",
        "vue" => "icons/file-types/vue.svg",
        "tf" | "tfvars" => "icons/file-types/terraform.svg",
        "graphql" | "gql" => "icons/file-types/graphql.svg",
        "lua" => "icons/file-types/lua.svg",
        "dart" => "icons/file-types/dart.svg",
        "astro" => "icons/file-types/astro.svg",
        "coffee" | "cson" => "icons/file-types/coffee.svg",
        "cr" => "icons/file-types/crystal.svg",
        "ex" | "exs" => "icons/file-types/elixir.svg",
        "elm" => "icons/file-types/elm.svg",
        "erl" | "hrl" => "icons/file-types/erlang.svg",
        "clj" | "cljs" | "cljc" | "edn" => "icons/file-types/clojure.svg",
        "hs" | "lhs" => "icons/file-types/haskell.svg",
        "hx" | "hxml" => "icons/file-types/haxe.svg",
        "jinja" | "jinja2" | "j2" => "icons/file-types/jinja.svg",
        "jl" => "icons/file-types/julia.svg",
        "ml" | "mli" => "icons/file-types/ocaml.svg",
        "pl" | "pm" => "icons/file-types/perl.svg",
        "prisma" => "icons/file-types/prisma.svg",
        "pug" | "jade" => "icons/file-types/pug.svg",
        "scala" | "sbt" | "sc" => "icons/file-types/scala.svg",
        "sol" => "icons/file-types/solidity.svg",
        "tex" | "sty" | "cls" => "icons/file-types/tex.svg",
        "xaml" => "icons/file-types/xaml.svg",
        "zig" => "icons/file-types/zig.svg",
        "nix" => "icons/file-types/nix.svg",
        "proto" => "icons/file-types/proto.svg",
        "diff" | "patch" => "icons/file-types/diff.svg",
        "exe" | "dll" | "so" | "dylib" => "icons/file-types/exe.svg",
        "lock" => "icons/file-types/lock.svg",
        _ => "icons/file-types/file.svg",
    }
}

#[cfg(test)]
pub(crate) fn visible_working_tree_entries(
    root: &Path,
    expanded_paths: &HashSet<PathBuf>,
) -> Vec<WorkingTreeEntry> {
    fn visit(
        directory: &Path,
        relative_directory: &Path,
        depth: usize,
        expanded_paths: &HashSet<PathBuf>,
        entries: &mut Vec<WorkingTreeEntry>,
    ) {
        let Ok(read_dir) = std::fs::read_dir(directory) else {
            return;
        };
        let mut children = read_dir
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name == ".git" {
                    return None;
                }
                let is_dir = entry.file_type().ok()?.is_dir();
                Some((entry.path(), name, is_dir))
            })
            .collect::<Vec<_>>();
        children.sort_by_key(|(_, name, is_dir)| (!*is_dir, name.to_lowercase()));

        for (absolute_path, name, is_dir) in children {
            let relative_path = relative_directory.join(&name);
            let expanded = is_dir && expanded_paths.contains(&absolute_path);
            let file_icon = (!is_dir).then(|| file_icon_for_name(&name));
            entries.push(WorkingTreeEntry {
                relative_path: relative_path.to_string_lossy().into_owned(),
                absolute_path: absolute_path.clone(),
                name,
                is_dir,
                is_ignored: false,
                file_icon,
                expanded,
                depth,
            });
            if expanded {
                visit(
                    &absolute_path,
                    &relative_path,
                    depth + 1,
                    expanded_paths,
                    entries,
                );
            }
        }
    }

    let mut entries = Vec::new();
    visit(root, Path::new(""), 0, expanded_paths, &mut entries);
    entries
}

/// The language name for a file, as understood by [`crate::md::highlight`].
/// Names the lexer does not know simply render unhighlighted.
pub(crate) fn file_highlighter_language(relative_path: &str) -> &'static str {
    let path = Path::new(relative_path);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let normalized_file_name = file_name.to_ascii_lowercase();

    // Lockfiles often have a generic `.lock` suffix (or no useful extension),
    // so resolve their actual serialization format before extension fallback.
    let lockfile_language = match normalized_file_name.as_str() {
        "bun.lock"
        | "composer.lock"
        | "conan.lock"
        | "deno.lock"
        | "flake.lock"
        | "npm-shrinkwrap.json"
        | "package-lock.json"
        | "package.resolved"
        | "packages.lock.json"
        | "pipfile.lock" => Some("json"),
        "cargo.lock" | "pdm.lock" | "poetry.lock" | "uv.lock" => Some("toml"),
        "chart.lock" | "gemfile.lock" | "pnpm-lock.yaml" | "podfile.lock" | "pubspec.lock"
        | "yarn.lock" => Some("yaml"),
        "mix.lock" => Some("elixir"),
        _ => None,
    };
    if let Some(language) = lockfile_language {
        return language;
    }

    if file_name == "Makefile" || file_name.starts_with("Makefile.") {
        return "make";
    }
    if normalized_file_name == "dockerfile" || normalized_file_name.starts_with("dockerfile.") {
        return "dockerfile";
    }

    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("rs") => "rust",
        Some("ts" | "mts" | "cts") => "typescript",
        Some("tsx") => "tsx",
        Some("js" | "jsx" | "mjs" | "cjs") => "javascript",
        Some("py" | "pyi") => "python",
        Some("go") => "go",
        Some("c") => "c",
        Some("h" | "hpp" | "hh" | "hxx" | "cc" | "cpp" | "cxx") => "cpp",
        Some("m" | "mm") => "objc",
        Some("java" | "kt" | "kts") => "java",
        Some("cs") => "csharp",
        Some("scala" | "sc") => "scala",
        Some("rb" | "rake" | "gemspec") => "ruby",
        Some("swift") => "swift",
        Some("json" | "jsonc" | "json5") => "json",
        Some("yaml" | "yml") => "yaml",
        Some("toml") => "toml",
        Some("ini" | "cfg" | "conf") => "ini",
        Some("sh" | "bash" | "zsh" | "fish") => "bash",
        Some("css" | "scss" | "sass" | "less") => "css",
        Some("html" | "htm" | "xml" | "svg" | "vue" | "svelte") => "html",
        Some("sql") => "sql",
        Some("diff" | "patch") => "diff",
        Some("md" | "markdown" | "mdx") => "markdown",
        _ => "text",
    }
}

/// Reads a file for the editor, returning its text and whether it can be saved.
///
/// One unbounded `read_to_string`, so callers keep it off the UI thread; the
/// only caller is [`Padu::read_right_panel_file_into_editor`].
pub(crate) fn read_right_panel_file(
    workspace: &padu_client::WorkspaceClient,
    project_path: &Path,
    relative_path: &str,
) -> (String, bool) {
    match workspace.request(padu_client::WorkspaceOperation::ReadTextFile {
        root: project_path.to_path_buf(),
        relative_path: PathBuf::from(relative_path),
    }) {
        Ok(padu_client::WorkspaceResult::TextFile { content }) => (content, true),
        Ok(_) => (
            tr!(
                "files.unable_to_edit",
                error = "the daemon returned an invalid file response"
            ),
            false,
        ),
        Err(error) => (
            tr!("files.unable_to_edit", error = error.to_string()),
            false,
        ),
    }
}
