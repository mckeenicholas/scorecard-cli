use std::fmt::Write as _;

use crate::pdf::table::ColumnDef;
use crate::pdf::text::TextAlign;
use crate::scorecard::TimeLimitInfo;

pub const ATTEMPT_COLUMNS: [ColumnDef<'static>; 5] = [
    ColumnDef::new("Attempt", 0.16, TextAlign::Center),
    ColumnDef::new("Scr", 0.15, TextAlign::Center),
    ColumnDef::new("Result", 0.39, TextAlign::Center),
    ColumnDef::new("Judge", 0.15, TextAlign::Center),
    ColumnDef::new("Comp", 0.15, TextAlign::Center),
];

/// Standard labels for regular attempt solve rows (avoids per-card integer-to-string allocations).
pub const ATTEMPT_LABELS: [&str; 5] = ["1", "2", "3", "4", "5"];

/// Specifications and precomputed geometry for the scorecard attempt table.
#[derive(Debug)]
pub struct AttemptTableSpec {
    pub attempt_count: usize,
    pub cutoff_attempts: usize,
    pub cutoff_banner: Option<String>,
    pub row_h: f32,
    pub total_table_h: f32,
    pub col_widths: [f32; 5],
    pub footer_text: Option<String>,
}

impl AttemptTableSpec {
    pub const HEADER_H: f32 = 12.0;
    pub const BANNER_H: f32 = 10.5;
    pub const BASE_ATTEMPT_ROWS: f32 = 6.0;

    pub fn build(
        inner_w: f32,
        available_h: f32,
        attempt_count: usize,
        time_limit_info: Option<TimeLimitInfo>,
    ) -> Self {
        let has_cutoff = time_limit_info
            .is_some_and(|info| info.cutoff_centiseconds.is_some() && info.cutoff_attempts > 0);

        let cutoff_attempts = if has_cutoff {
            time_limit_info.map_or(0, |info| info.cutoff_attempts)
        } else {
            0
        };

        let footer_text = time_limit_info.and_then(|info| {
            info.limit_centiseconds.map(|limit| {
                let mut text = String::with_capacity(32);
                if info.is_cumulative {
                    let _ = write!(text, "Time limit: {limit} cumulative");
                } else {
                    let _ = write!(text, "Time limit: {limit}");
                }
                text
            })
        });

        let footer_reserve = if footer_text.is_some() { 12.0 } else { 0.0 };
        let usable_h = available_h - footer_reserve;
        let banner_count_5 = if has_cutoff { 2.0 } else { 1.0 };
        let row_h = ((usable_h - Self::HEADER_H - (banner_count_5 * Self::BANNER_H))
            / Self::BASE_ATTEMPT_ROWS)
            .max(13.5);

        let cutoff_banner = has_cutoff.then(|| {
            let cutoff = time_limit_info
                .and_then(|info| info.cutoff_centiseconds)
                .unwrap_or_default();
            let format_name = if attempt_count <= 3 {
                "mean"
            } else {
                "average"
            };
            let mut banner = String::with_capacity(80);
            let _ = write!(
                banner,
                "-------- Must have solve under {cutoff} to complete {format_name} --------"
            );
            banner
        });

        let total_banners = if has_cutoff { 2.0 } else { 1.0 };
        let total_attempts = f32::from(u16::try_from(attempt_count + 1).unwrap_or(6));
        let total_table_h =
            Self::HEADER_H + (total_attempts * row_h) + (total_banners * Self::BANNER_H);

        let col_widths = ATTEMPT_COLUMNS.map(|col| inner_w * col.ratio);

        Self {
            attempt_count,
            cutoff_attempts,
            cutoff_banner,
            row_h,
            total_table_h,
            col_widths,
            footer_text,
        }
    }
}
