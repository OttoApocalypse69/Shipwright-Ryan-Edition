use sre_core::{CompatibilityState, GameCatalog, GameId};

const CATALOG: &str = include_str!("../../../packages/game-catalog/catalog.v1.json");

#[test]
fn bundled_catalog_is_semantically_valid_and_has_initial_targets_in_order() {
    let catalog = GameCatalog::from_json(CATALOG).expect("bundled catalog must be valid");
    let ids = catalog
        .games
        .iter()
        .map(|game| game.id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        ids,
        [
            "zelda-oot",
            "zelda-mm",
            "zelda-skyward-sword",
            "zelda-botw",
            "zelda-totk",
            "zelda-echoes-of-wisdom",
            "animal-crossing-new-horizons",
        ]
    );
}

#[test]
fn only_verified_paths_are_marked_supported() {
    let catalog = GameCatalog::from_json(CATALOG).unwrap();
    let oot = catalog.game(&GameId::new("zelda-oot").unwrap()).unwrap();
    assert_eq!(
        oot.variants[0].runtime_candidates[0].compatibility,
        CompatibilityState::Supported
    );

    let mm = catalog.game(&GameId::new("zelda-mm").unwrap()).unwrap();
    assert_eq!(
        mm.variants[0].runtime_candidates[0].compatibility,
        CompatibilityState::Supported
    );

    let botw = catalog.game(&GameId::new("zelda-botw").unwrap()).unwrap();
    assert_eq!(
        botw.variants[0].runtime_candidates[0].compatibility,
        CompatibilityState::Supported
    );

    let totk = catalog.game(&GameId::new("zelda-totk").unwrap()).unwrap();
    assert_eq!(
        totk.variants[0].runtime_candidates[0].compatibility,
        CompatibilityState::Supported
    );

    let eow = catalog
        .game(&GameId::new("zelda-echoes-of-wisdom").unwrap())
        .unwrap();
    assert_eq!(
        eow.variants[0].runtime_candidates[0].compatibility,
        CompatibilityState::Supported
    );

    let animal_crossing = catalog
        .game(&GameId::new("animal-crossing-new-horizons").unwrap())
        .unwrap();
    assert_eq!(
        animal_crossing.variants[0].runtime_candidates[0].compatibility,
        CompatibilityState::Supported
    );

    let skyward_sword = catalog
        .game(&GameId::new("zelda-skyward-sword").unwrap())
        .unwrap();
    assert_eq!(
        skyward_sword.variants[0].runtime_candidates[0].compatibility,
        CompatibilityState::Supported
    );
    assert_eq!(
        skyward_sword.variants[0].original_platform,
        sre_core::OriginalPlatform::Switch
    );
    assert_eq!(
        skyward_sword.variants[0]
            .preferred_runtime
            .as_ref()
            .unwrap()
            .as_str(),
        "switch-runtime"
    );

    for game in catalog.games.iter().filter(|game| {
        !matches!(
            game.id.as_str(),
            "zelda-oot"
                | "zelda-mm"
                | "zelda-botw"
                | "zelda-totk"
                | "zelda-echoes-of-wisdom"
                | "animal-crossing-new-horizons"
                | "zelda-skyward-sword"
        )
    }) {
        assert!(
            game.variants
                .iter()
                .flat_map(|variant| &variant.runtime_candidates)
                .all(|candidate| candidate.compatibility != CompatibilityState::Supported)
        );
    }
}
