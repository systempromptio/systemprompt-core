use systemprompt_bridge::window_state::{
    MIN_HEIGHT, MIN_WIDTH, WindowGeometry, WorkArea, clamp_size, restore,
};

fn primary() -> WorkArea {
    WorkArea {
        x: 0,
        y: 0,
        width: 1920,
        height: 1080,
    }
}

fn secondary_left() -> WorkArea {
    WorkArea {
        x: -1920,
        y: 0,
        width: 1920,
        height: 1080,
    }
}

fn geom(x: i32, y: i32) -> WindowGeometry {
    WindowGeometry {
        x,
        y,
        width: 1100,
        height: 760,
        maximized: false,
    }
}

#[test]
fn a_window_on_the_primary_display_is_restored_unchanged() {
    let saved = geom(120, 80);
    assert_eq!(restore(saved, &[primary()]), Some(saved));
}

#[test]
fn a_window_saved_on_a_display_that_is_gone_falls_back_to_os_centring() {
    let saved = geom(-1800, 40);
    assert_eq!(restore(saved, &[primary(), secondary_left()]), Some(saved));
    assert_eq!(restore(saved, &[primary()]), None);
}

#[test]
fn a_window_only_partly_on_screen_is_still_reachable_and_kept() {
    let saved = geom(1870, 100);
    assert_eq!(restore(saved, &[primary()]), Some(saved));
}

#[test]
fn a_window_entirely_past_the_right_edge_is_rejected() {
    assert_eq!(restore(geom(1920, 100), &[primary()]), None);
}

#[test]
fn restoring_with_no_displays_reported_falls_back_to_os_centring() {
    assert_eq!(restore(geom(10, 10), &[]), None);
}

#[test]
fn a_geometry_saved_below_the_minimum_is_grown_back_to_it() {
    let saved = WindowGeometry {
        x: 0,
        y: 0,
        width: 320,
        height: 240,
        maximized: false,
    };
    let restored = restore(saved, &[primary()]).expect("on-screen");
    assert_eq!(restored.width, MIN_WIDTH);
    assert_eq!(restored.height, MIN_HEIGHT);
    assert_eq!(restored.x, 0);
}

#[test]
fn maximized_state_survives_a_restore() {
    let saved = WindowGeometry {
        maximized: true,
        ..geom(0, 0)
    };
    assert!(restore(saved, &[primary()]).expect("on-screen").maximized);
}

#[test]
fn clamp_leaves_a_size_at_or_above_the_minimum_alone() {
    assert_eq!(clamp_size(1100, 760), (1100, 760));
    assert_eq!(clamp_size(MIN_WIDTH, MIN_HEIGHT), (MIN_WIDTH, MIN_HEIGHT));
}
