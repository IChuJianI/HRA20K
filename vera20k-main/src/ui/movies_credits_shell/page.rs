//! Dialog `0x101` Movies & Credits page table.

use crate::ui::shell::descriptor::DialogId;
use crate::ui::shell::menu_page::{self, MenuPageButtonSpec, MenuPageLayout, MenuPageSpec};

/// RT_DIALOG `0x101` owner-draw buttons with the results written by dialog
/// proc `0x0052D790` (jump table `0x0052D848`, index bytes `0x0052D85C`).
/// VERA's License entry: not a retail control, on the free row under View
/// Credits. Its result is VERA's own; the retail proc's jump table has no
/// entry for it.
pub const LICENSE_CONTROL: u16 = 0x7F70;

pub const MOVIES_CREDITS_PAGE: MenuPageSpec = MenuPageSpec {
    dialog: DialogId(0x0101),
    title_key: "GUI:MoviesAndCredits",
    stacked: &[
        MenuPageButtonSpec {
            id: 0x068D,
            dlu_top: 122,
            csf_key: "GUI:SneakPeeks",
            tooltip_key: "STT:OptionsButtonSneak",
            result: Some(0x0D),
        },
        MenuPageButtonSpec {
            id: 0x068E,
            dlu_top: 149,
            csf_key: "GUI:PlayMovies",
            tooltip_key: "STT:OptionsButtonMovies",
            result: Some(0x0E),
        },
        MenuPageButtonSpec {
            id: 0x068F,
            dlu_top: 176,
            csf_key: "GUI:ViewCredits",
            tooltip_key: "STT:OptionsButtonCredits",
            result: Some(0x0F),
        },
        MenuPageButtonSpec {
            id: LICENSE_CONTROL,
            dlu_top: 203,
            csf_key: "VERA:LegalPage",
            tooltip_key: "VERA:LegalPageHelp",
            result: Some(0x10),
        },
    ],
    back: MenuPageButtonSpec {
        id: 0x0686,
        dlu_top: 346,
        csf_key: "GUI:MainMenu",
        tooltip_key: "STT:OptionsButtonBack",
        result: Some(0x12),
    },
};

/// Main-loop state a `0x101` command selects.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoviesCreditsAction {
    /// State `0xD`: play RENEGADE.BIK, then recreate `0x101`.
    SneakPeeks,
    /// State `0xE`: the movie list `0x129`.
    PlayMovies,
    /// State `0xF`: Show_Credits, then recreate `0x101`.
    ViewCredits,
    /// VERA: the license and source page, a copy of the credits roll.
    License,
    /// State `0x12`: the main menu `0xE2`.
    MainMenu,
}

pub fn action_for_control(id: u16) -> Option<MoviesCreditsAction> {
    let result = MOVIES_CREDITS_PAGE.button(id)?.result?;
    Some(match result {
        0x0D => MoviesCreditsAction::SneakPeeks,
        0x0E => MoviesCreditsAction::PlayMovies,
        0x0F => MoviesCreditsAction::ViewCredits,
        0x10 => MoviesCreditsAction::License,
        0x12 => MoviesCreditsAction::MainMenu,
        _ => return None,
    })
}

pub fn compute_layout(screen_w: u32, screen_h: u32) -> MenuPageLayout {
    menu_page::compute_layout(&MOVIES_CREDITS_PAGE, screen_w, screen_h)
}

#[cfg(all(test, tests_ui))]
mod tests {
    use super::*;
    use crate::ui::shell::geom::RectPx;

    #[test]
    fn commands_match_dialog_proc_0x52d790() {
        assert_eq!(
            action_for_control(0x068D),
            Some(MoviesCreditsAction::SneakPeeks)
        );
        assert_eq!(
            action_for_control(0x068E),
            Some(MoviesCreditsAction::PlayMovies)
        );
        assert_eq!(
            action_for_control(0x068F),
            Some(MoviesCreditsAction::ViewCredits)
        );
        // VERA's own row under View Credits; retail's proc has no result 0x10.
        assert_eq!(
            action_for_control(LICENSE_CONTROL),
            Some(MoviesCreditsAction::License)
        );
        assert_eq!(
            action_for_control(0x0686),
            Some(MoviesCreditsAction::MainMenu)
        );
        // 0x687..0x68C share the proc's default index 4: no state is written.
        for unused in 0x0687..=0x068C {
            assert_eq!(action_for_control(unused), None);
        }
    }

    #[test]
    fn page_rows_share_the_single_player_geometry() {
        let movies = compute_layout(800, 600);
        let single = crate::ui::single_player_shell::compute_layout(800, 600);
        assert_eq!(movies.title, single.title);
        assert_eq!(movies.status_help, single.status_help);
        let rects = |layout: &MenuPageLayout| {
            layout
                .buttons
                .iter()
                .map(|button| button.rect)
                .collect::<Vec<RectPx>>()
        };
        // The three retail rows keep the Single Player page's geometry; VERA's
        // License row and the Back tile follow them.
        let movies_rects = rects(&movies);
        let single_rects = rects(&single);
        assert_eq!(movies_rects[..3], single_rects[..3]);
        assert_eq!(
            movies_rects.last(),
            single_rects.last(),
            "Back keeps the final tile"
        );
        assert_eq!(
            movies.button_rect(0x068D),
            Some(RectPx::new(644, 199, 156, 42))
        );
        assert_eq!(
            movies.button_rect(0x0686),
            Some(RectPx::new(644, 535, 156, 42))
        );
        let view_credits = movies.button_rect(0x068F).expect("View Credits");
        let license = movies.button_rect(LICENSE_CONTROL).expect("License");
        assert!(license.y > view_credits.y);
        assert!(license.y + license.h <= movies.button_rect(0x0686).expect("Back").y);
        assert_eq!(license.w, view_credits.w);
    }
}
