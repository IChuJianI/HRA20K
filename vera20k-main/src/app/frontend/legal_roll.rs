//! VERA's License page: the shell's legal notices and source addresses,
//! scrolled like Show_Credits. The retail install has no such page; the
//! startup splash's notice is the license's Appropriate Legal Notice, and
//! this page keeps the same text reachable after it, with the modification
//! notice a port must show.

use crate::i18n::{tr, tr_fill};
use crate::render::bit_font::BitFont;
use crate::ui::movies_credits_shell::{CreditLine, CreditsLayoutFlags};

/// Where this build's corresponding source is offered. A modified version's
/// notice names the modified source, not only upstream. Also drawn on the
/// startup splash under VERA20K's own notice.
pub(crate) const PORT_SOURCE: &str = "github.com/IChuJianI/HRA20K";

/// Line pitch and text width of the credits page, in logical pixels. The
/// width leaves the roll's 520 px box a margin on both sides.
const LINE_ADVANCE: i32 = 0x10;
const TEXT_WIDTH: i32 = 500;

const CENTERED: u32 = 0x148;
const LEFT: u32 = 0x48;

/// The page's lines, from the top of the roll's off-screen start.
pub(crate) fn legal_lines(font: &BitFont, surface_height: i32) -> Vec<CreditLine> {
    let mut lines = Vec::new();
    let mut y = surface_height + 2;
    let centered = |lines: &mut Vec<CreditLine>, y: &mut i32, text: &str| {
        push(lines, y, text, true);
    };
    let paragraph = |lines: &mut Vec<CreditLine>, y: &mut i32, text: &str| {
        for part in wrap(font, text) {
            push(lines, y, &part, false);
        }
    };

    centered(&mut lines, &mut y, &tr("VERA:LegalPageTitle"));
    blank(&mut y);
    paragraph(&mut lines, &mut y, &tr("VERA:LegalPortCopyright"));
    paragraph(&mut lines, &mut y, &tr("VERA:LegalModification"));
    blank(&mut y);
    paragraph(&mut lines, &mut y, &tr("VERA:LegalCopyright"));
    paragraph(&mut lines, &mut y, &tr("VERA:LegalShare"));
    paragraph(
        &mut lines,
        &mut y,
        &tr_fill("VERA:LegalSource", &[("source", &env!("VERA20K_SOURCE"))]),
    );
    paragraph(
        &mut lines,
        &mut y,
        &tr_fill("VERA:LegalPortSource", &[("source", &PORT_SOURCE)]),
    );
    blank(&mut y);
    paragraph(&mut lines, &mut y, &tr("VERA:LegalThirdParty"));
    blank(&mut y);
    paragraph(&mut lines, &mut y, &tr("VERA:LegalGameFiles"));
    blank(&mut y);
    paragraph(&mut lines, &mut y, &tr("VERA:LegalFullText"));
    blank(&mut y);
    centered(&mut lines, &mut y, &tr("VERA:LegalBack"));
    lines
}

fn push(lines: &mut Vec<CreditLine>, y: &mut i32, text: &str, centered: bool) {
    let text = text.trim();
    if text.is_empty() {
        blank(y);
        return;
    }
    lines.push(CreditLine {
        text: text.to_owned(),
        column: if centered { 4 } else { 0 },
        y: *y,
        flags: CreditsLayoutFlags(if centered { CENTERED } else { LEFT }),
    });
    *y += LINE_ADVANCE;
}

fn blank(y: &mut i32) {
    *y += LINE_ADVANCE;
}

/// Break `text` to the box's width, at spaces where possible and between
/// characters otherwise (Chinese has no spaces).
fn wrap(font: &BitFont, text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    for ch in text.chars() {
        let mut next = current.clone();
        next.push(ch);
        if font.text_width(&next) as i32 <= TEXT_WIDTH {
            current = next;
            continue;
        }
        match current.rfind(' ') {
            Some(space) => {
                let head = current[..space].to_string();
                let tail = current[space + 1..].to_string();
                if !head.is_empty() {
                    out.push(head);
                }
                current = tail;
                current.push(ch);
            }
            None => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
                current.push(ch);
            }
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}