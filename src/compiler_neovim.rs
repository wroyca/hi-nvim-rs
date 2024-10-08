use std::fmt::Write;

use anyhow::Result;

use crate::colorscheme::{Colorscheme, Theme};
use crate::configuration::Highlight;
use crate::error::Error;

struct Compiler<'c> {
    colorscheme: &'c Colorscheme<'c>,
    program: String,
    indent: u8,
}

impl Compiler<'_> {
    fn indent(&mut self) {
        self.indent = self.indent.saturating_add(1);
    }

    fn dedent(&mut self) {
        self.indent = self.indent.saturating_sub(1);
    }

    fn write_indent(&mut self) {
        self.program += &str::repeat(" ", usize::from(self.indent).saturating_mul(2));
    }

    fn compile_themes(&mut self, theme: &Theme) -> Result<()> {
        for (namespace, theme_colors) in &theme.colors {
            self.write_indent();
            writeln!(self.program, "local {namespace} = {{",)?;

            self.indent();
            for (name, theme_color) in theme_colors {
                self.write_indent();
                write!(self.program, r#"["{name}"] = "{theme_color}""#)?;
                writeln!(self.program, ",")?;
            }
            self.dedent();

            self.write_indent();
            writeln!(self.program, "}}")?;
        }

        Ok(())
    }

    fn compile_highlight_group(&mut self, name: &str, highlight: &Highlight) -> Result<()> {
        self.write_indent();
        write!(&mut self.program, r#"hi(0, "{name}", {{ "#,)?;

        fn write_unquoted(program: &mut String, name: &str, value: Option<&str>) {
            if let Some(value) = value {
                write!(program, "{name} = {value}, ",).unwrap()
            }
        }

        fn write_quoted(program: &mut String, name: &str, value: Option<&str>) {
            if let Some(value) = value {
                write!(program, r#"{name} = "{value}", "#,).unwrap();
            }
        }

        if highlight.link.is_some() {
            write_quoted(&mut self.program, "link", highlight.link.as_deref());
        } else {
            if let Some(fg) = highlight.fg {
                write!(
                    self.program,
                    r#"fg = {ns}["{name}"], "#,
                    ns = fg.theme_namespace,
                    name = fg.element_name,
                )?;
            }
            if let Some(bg) = highlight.bg {
                write!(
                    self.program,
                    "bg = {ns}.{name}, ",
                    ns = bg.theme_namespace,
                    name = bg.element_name,
                )?;
            }
            if let Some(sp) = highlight.sp {
                write!(
                    self.program,
                    "sp = {ns}.{name}, ",
                    ns = sp.theme_namespace,
                    name = sp.element_name,
                )?;
            }

            for style in highlight.gui_styles_iter() {
                write_unquoted(&mut self.program, style, Some("true"));
            }
        }

        writeln!(self.program, "}})")?;

        Ok(())
    }

    fn compile_inner(&mut self, theme: &Theme) -> Result<()> {
        self.write_indent();
        writeln!(self.program, "-- Colors")?;

        self.compile_themes(theme)?;

        writeln!(self.program, "")?;
        self.write_indent();
        writeln!(self.program, "-- Highlights")?;

        for (name, highlight) in &self.colorscheme.highlights {
            self.compile_highlight_group(name, highlight)?;
        }

        Ok(())
    }
}

/// Compile a color scheme to a Neovim configuration.
// This should probably be infallible.
pub fn compile(colorscheme: &Colorscheme) -> std::result::Result<String, Error> {
    let mut compiler = Compiler {
        colorscheme,
        program: String::with_capacity(8192),
        indent: 0,
    };

    write!(
        compiler.program,
        r#"if vim.g.colors_name ~= nil then vim.cmd("highlight clear") end
vim.g.colors_name = "{name}"

-- Highlight groups
local hi = vim.api.nvim_set_hl
"#,
        name = colorscheme.name
    )
    .map_err(|_| Error::CompilationFailed)?;

    compiler.indent();

    writeln!(compiler.program).map_err(|_| Error::CompilationFailed)?;

    // Directly compile themes and highlights for dark mode
    writeln!(compiler.program, r#"if vim.o.background == "dark" then"#)
        .map_err(|_| Error::CompilationFailed)?;
    compiler
        .compile_inner(&colorscheme.dark_theme)
        .map_err(|_| Error::CompilationFailed)?;
    compiler.dedent();

    // Directly compile themes and highlights for light mode
    writeln!(compiler.program, r#"end"#).map_err(|_| Error::CompilationFailed)?;
    writeln!(compiler.program, r#"if vim.o.background == "light" then"#)
        .map_err(|_| Error::CompilationFailed)?;
    compiler.indent();
    compiler
        .compile_inner(&colorscheme.light_theme)
        .map_err(|_| Error::CompilationFailed)?;
    compiler.dedent();
    writeln!(compiler.program, "end").map_err(|_| Error::CompilationFailed)?;

    compiler.dedent();

    Ok(compiler.program)
}
