//! hip4-tui's `engine/src/tests/builders.rs`, ported to the matrix as hl-hydromancer reads it,
//! and Hydromancer's documented example of `outcome-builder-volume-daily` (packed), whose
//! windows were summed by hand.

use hl_core::Dec;
use hl_hydromancer::{DailySeries, VolumeDaily};

use super::*;

const OUTCOME: &str = "0xab5dbc057628bc18523c4cdfc0e1e2ebdbecb704";
const DFU: &str = "0x6967d7acd1ec5f210e9590666498e0cc5d13a843";
const DOCUMENTED: &str = include_str!("../../fixtures/dashboard_volume_daily.json");

fn dec(text: &str) -> Dec {
    Dec::parse(text).unwrap()
}

fn address(text: &str) -> Address {
    text.parse().unwrap()
}

fn matrix(builders: Vec<Option<&str>>, volume: Vec<Vec<&str>>, fills: Vec<Vec<u64>>) -> VolumeDaily {
    let days = volume[0].len();
    let dates: Vec<String> = (0..days).map(|day| format!("2026-01-{:02}", day + 1)).collect();
    VolumeDaily {
        start_date: dates.first().cloned().unwrap_or_default(),
        end_date: dates.last().cloned().unwrap_or_default(),
        builders: builders.into_iter().map(|one| one.map(address)).collect(),
        dates,
        series: DailySeries {
            unique_users: fills.iter().map(|row| row.iter().map(|count| count / 10).collect()).collect(),
            builder_fee: volume.iter().map(|row| row.iter().map(|_| dec("1.00")).collect()).collect(),
            volume: volume.iter().map(|row| row.iter().map(|one| dec(one)).collect()).collect(),
            fills,
        },
    }
}

fn eight_days() -> VolumeDaily {
    matrix(
        vec![Some(OUTCOME), Some(DFU), None],
        vec![
            vec!["1000", "10", "10", "10", "10", "10", "10", "10"],
            vec!["0", "100", "100", "100", "100", "100", "100", "100"],
            vec!["0", "50", "50", "50", "50", "50", "50", "50"],
        ],
        vec![vec![100, 1, 1, 1, 1, 1, 1, 1], vec![0, 10, 10, 10, 10, 10, 10, 10], vec![0, 5, 5, 5, 5, 5, 5, 5]],
    )
}

fn board() -> Board {
    Board::of(&eight_days()).unwrap()
}

fn ours(board: &Board, window: Window) -> &Standing {
    board.find(window, address(OUTCOME)).unwrap()
}

#[test]
fn a_window_is_the_newest_days_and_not_the_whole_matrix() {
    let board = board();
    assert_eq!(ours(&board, Window::All).volume, dec("1070"));
    assert_eq!(ours(&board, Window::Week).volume, dec("70"));
    assert_eq!(board.total(Window::All), dec("2120"));
    assert_eq!(board.total(Window::Week), dec("1120"));
}

#[test]
fn a_window_longer_than_the_matrix_is_the_whole_matrix() {
    let board = board();
    assert_eq!(board.table(Window::Month), board.table(Window::All), "a short history is all the history there is");
}

#[test]
fn the_shortest_window_is_the_newest_column_and_nothing_before_it() {
    let board = board();
    assert_eq!(ours(&board, Window::Day).volume, dec("10"));
    assert_eq!(board.total(Window::Day), dec("160"));
}

#[test]
fn the_largest_builder_leads_and_the_rest_follow_it_down() {
    let board = board();
    let ranked: Vec<(Option<Address>, Option<u32>)> =
        board.table(Window::Week).iter().map(|row| (row.builder, row.rank)).collect();
    assert_eq!(
        ranked,
        vec![(Some(address(DFU)), Some(1)), (None, None), (Some(address(OUTCOME)), Some(2))],
        "sorted by volume, and the bucket with no builder holds no place"
    );
}

#[test]
fn the_bucket_with_no_builder_keeps_its_place_in_the_order() {
    let board = board();
    let week = board.table(Window::Week);
    let order: Vec<Dec> = week.iter().map(|row| row.volume).collect();
    assert_eq!(order, vec![dec("700"), dec("350"), dec("70")]);
    assert!(week[1].is_direct());
    assert_eq!(week[1].rank, None);
}

#[test]
fn a_share_is_of_the_whole_network_and_the_shares_add_up() {
    let board = board();
    let week = board.table(Window::Week);
    let total: u32 = week.iter().map(|row| row.share).sum();
    let rows = u32::try_from(week.len()).unwrap();
    assert!((SHARE_UNITS - rows..=SHARE_UNITS).contains(&total), "shares came to {total}");
    let direct = week.iter().find(|row| row.is_direct()).unwrap();
    assert_eq!(direct.share, 3125, "350 of 1120");
}

#[test]
fn users_are_the_days_peak_because_they_cannot_be_summed() {
    let daily = matrix(vec![Some(OUTCOME)], vec![vec!["10", "10", "10"]], vec![vec![10, 50, 30]]);
    assert_eq!(Board::of(&daily).unwrap().table(Window::All)[0].peak_users, 5);
}

#[test]
fn the_bucket_is_the_bucket_however_the_provider_spelt_it() {
    for spelling in ["null", "NULL"] {
        let answer = format!(
            r#"{{"startDate":"2026-01-01","endDate":"2026-01-02","builders":["{OUTCOME}","{spelling}"],
            "dates":["2026-01-01","2026-01-02"],"series":{{"volume":[["10","10"],["90","90"]],
            "fills":[[1,1],[9,9]],"uniqueUsers":[[1,1],[1,1]],"builderFee":[["0","0"],["0","0"]]}}}}"#
        );
        let daily: VolumeDaily = serde_json::from_str(&answer).unwrap();
        let board = Board::of(&daily).unwrap();
        let all = board.table(Window::All);
        assert!(all[0].is_direct(), "`{spelling}` is not a builder: {:?}", all[0].builder);
        assert_eq!(all[0].rank, None, "and it holds no place in the ranking");
        assert_eq!(all[1].rank, Some(1), "the first real builder leads");
    }
}

#[test]
fn a_builder_with_nothing_in_the_window_is_not_a_row_in_it() {
    let daily = matrix(
        vec![Some(OUTCOME), Some(DFU)],
        vec![vec!["100", "0", "0"], vec!["0", "0", "0"]],
        vec![vec![1, 0, 0], vec![0, 0, 0]],
    );
    let board = Board::of(&daily).unwrap();
    let all = board.table(Window::All);
    assert_eq!(all.len(), 1, "a row of zeroes is not a builder to rank");
    assert_eq!(all[0].builder, Some(address(OUTCOME)));
    assert!(board.table(Window::Day).is_empty(), "nobody traded on the newest day");
}

#[test]
fn a_ragged_answer_is_refused_rather_than_folded_short() {
    let mut daily = eight_days();
    daily.series.volume[1].pop();
    assert_eq!(Board::of(&daily), Err(Ragged), "a short row would under-report the builder it belongs to");
    let mut daily = eight_days();
    daily.series.builder_fee.pop();
    assert_eq!(Board::of(&daily), Err(Ragged));
}

#[test]
fn the_top_of_the_board_keeps_the_bucket_with_no_builder() {
    let board = board();
    let top = board.top(Window::Week, 1);
    assert_eq!(top.len(), 2, "one ranked builder, and direct beside it");
    assert_eq!(top[0].rank, Some(1));
    assert!(top[1].is_direct());
    assert_eq!(board.builders(Window::Week), 2);
}

#[test]
fn a_builder_is_found_however_its_address_was_cased() {
    let board = board();
    assert!(board.find(Window::All, address(&OUTCOME.to_uppercase())).is_some());
    assert!(board.find(Window::All, Address::ZERO).is_none());
}

#[test]
fn the_window_asked_for_ends_today_and_starts_a_year_back() {
    let now = At::parse("2026-09-17T12:00:00Z").unwrap();
    assert_eq!(Window::since(now), "2025-09-19");
}

#[test]
fn the_windows_are_the_four_the_board_offers_shortest_first() {
    assert_eq!(Window::ALL.map(Window::label), ["24h", "7d", "30d", "all"]);
    assert_eq!(Window::ALL.map(Window::days), [1, 7, 30, 364]);
}

#[test]
fn the_documented_answer_ranks_its_one_builder_beside_direct_trading() {
    let daily: VolumeDaily = serde_json::from_str(DOCUMENTED).unwrap();
    let board = Board::of(&daily).unwrap();
    assert_eq!(board.dates, ["2026-06-01", "2026-06-02", "2026-06-03"]);

    let all = board.table(Window::All);
    let builder = &all[0];
    assert_eq!(builder.builder, Some(address(OUTCOME)));
    assert_eq!(
        (builder.volume, builder.fees, builder.fills, builder.peak_users),
        (dec("361000"), dec("722"), 2370, 165)
    );
    assert_eq!((builder.rank, builder.share), (Some(1), 6955), "361,000 of 519,000");
    assert_eq!(builder.daily, [dec("120000"), dec("98000"), dec("143000")], "a day a figure, oldest first");
    assert!(all[1].is_direct());
    assert_eq!((all[1].volume, all[1].fees, all[1].share), (dec("158000"), dec("316"), 3044));
    assert_eq!((board.total(Window::All), board.fees(Window::All)), (dec("519000"), dec("1038")));

    let day = board.table(Window::Day);
    assert_eq!((day[0].volume, day[0].share, day[0].fees), (dec("143000"), 7526, dec("286")));
    assert_eq!((day[1].volume, day[1].share), (dec("47000"), 2473));
    assert_eq!(day[0].daily, [dec("143000")]);
    assert_eq!(board.table(Window::Week), board.table(Window::All), "three days of history");
}
