use ftep_core::{CompatibilityState, GameCatalog, GameId};

const CATALOG: &str = include_str!("../../../packages/game-catalog/catalog.v1.json");

#[test]
fn bundled_catalog_is_semantically_valid_and_has_initial_targets_in_order() {
    let catalog = GameCatalog::from_json(CATALOG).expect("bundled catalog must be valid");
    let ids = catalog
        .games
        .iter()
        .map(|game| game.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids, ["zelda-oot", "zelda-mm", "zelda-botw", "zelda-totk"]);
}

#[test]
fn only_the_current_verified_oot_path_is_marked_supported() {
    let catalog = GameCatalog::from_json(CATALOG).unwrap();
    let oot = catalog.game(&GameId::new("zelda-oot").unwrap()).unwrap();
    assert_eq!(
        oot.variants[0].runtime_candidates[0].compatibility,
        CompatibilityState::Supported
    );

    for game in catalog.games.iter().skip(1) {
        assert!(
            game.variants
                .iter()
                .flat_map(|variant| &variant.runtime_candidates)
                .all(|candidate| candidate.compatibility != CompatibilityState::Supported)
        );
    }
}
