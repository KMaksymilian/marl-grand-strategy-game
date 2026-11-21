mod common;

use common::material;
use simulation_core::economy::storage_order::StorageOrderError;
use simulation_core::economy::{
    InventoryAddResult, InventoryRejection, ResourceCatalog, ResourceId, ResourceInventory,
    ResourceType,
};
use std::collections::HashMap;

fn custom_catalog() -> ResourceCatalog {
    ResourceCatalog::from_toml(
        r#"
        [[resources]]
        id = "salt"
        name = "Salt"
        tier = 1
        category = "crafted_good"

        [[resources]]
        id = "spice"
        name = "Spice"
        tier = 1
        category = "crafted_good"
        "#,
    )
    .unwrap()
}

#[test]
fn independent_inventories_apply_their_own_catalog_order_to_overflow() {
    let catalog = custom_catalog();
    let salt = catalog.resolve("salt").unwrap();
    let spice = catalog.resolve("spice").unwrap();
    let mut salt_first = ResourceInventory::with_resource_catalog(5, &catalog);
    let mut spice_first = ResourceInventory::with_resource_catalog(5, &catalog);
    let reverse_order = vec![spice.clone(), salt.clone()];
    spice_first.set_storage_order(&reverse_order).unwrap();
    assert_eq!(salt_first.storage_order(), catalog.materials());
    assert_eq!(spice_first.storage_order(), reverse_order);

    for inventory in [&mut salt_first, &mut spice_first] {
        assert_eq!(inventory.add(salt.clone(), 1).accepted, 1);
    }
    let batch = HashMap::from([(salt.clone(), 3), (spice.clone(), 3)]);
    assert_eq!(
        salt_first.add_batch(batch.clone()),
        vec![InventoryRejection {
            resource: spice.clone(),
            amount: 2,
        }],
    );
    assert_eq!(
        spice_first.add_batch(batch),
        vec![InventoryRejection {
            resource: salt.clone(),
            amount: 2,
        }],
    );
    assert_eq!(salt_first.amount(salt.clone()), 4);
    assert_eq!(salt_first.amount(spice.clone()), 1);
    assert_eq!(spice_first.amount(salt), 2);
    assert_eq!(spice_first.amount(spice), 3);
    assert_eq!(salt_first.total_amount(), 5);
    assert_eq!(spice_first.total_amount(), 5);
}

#[test]
fn invalid_orders_preserve_inventory_policy_and_stored_goods() {
    let catalog = custom_catalog();
    let salt = catalog.resolve("salt").unwrap();
    let spice = catalog.resolve("spice").unwrap();
    let mut inventory = ResourceInventory::with_resource_catalog(10, &catalog);
    let preferred = vec![spice.clone(), salt.clone()];
    inventory.set_storage_order(&preferred).unwrap();
    inventory.add(salt.clone(), 3);
    let stock = inventory.resources().clone();
    let unknown = material("wood");
    let invalid = [
        (
            vec![salt.clone()],
            StorageOrderError::MissingMaterial(spice),
        ),
        (
            vec![salt.clone(), salt.clone()],
            StorageOrderError::DuplicateMaterial(salt),
        ),
        (
            vec![ResourceType::Labour],
            StorageOrderError::LabourIsNotMaterial,
        ),
        (
            vec![unknown.clone()],
            StorageOrderError::UnknownMaterial(unknown),
        ),
    ];
    for (order, error) in invalid {
        assert_eq!(inventory.validate_storage_order(&order), Err(error.clone()));
        assert_eq!(inventory.set_storage_order(order), Err(error));
        assert_eq!(inventory.storage_order(), preferred);
        assert_eq!(inventory.resources(), &stock);
    }
}

#[test]
fn batch_rejections_preserve_quantities_above_the_inventory_counter_limit() {
    let mut inventory = ResourceInventory::new(u32::MAX);
    let wood = material("wood");
    let requested = u64::from(u32::MAX) + 17;
    let rejections = inventory.add_batch(HashMap::from([(wood.clone(), requested)]));

    assert_eq!(inventory.amount(wood.clone()), u32::MAX);
    assert_eq!(
        rejections,
        vec![InventoryRejection {
            resource: wood,
            amount: 17,
        }],
    );
    assert_eq!(inventory.total_amount() + rejections[0].amount, requested);
}

#[test]
fn unknown_materials_cannot_enter_through_single_or_batch_additions() {
    let mut inventory = ResourceInventory::new(10);
    let unknown = ResourceType::Material(ResourceId::new("ScenarioOnlyMaterial").unwrap());
    assert!(!inventory.knows_material(&unknown));
    assert_eq!(
        inventory.add(unknown.clone(), 4),
        InventoryAddResult {
            accepted: 0,
            rejected: 4,
        },
    );
    assert_eq!(
        inventory.add_batch(HashMap::from([(unknown.clone(), 5)])),
        vec![InventoryRejection {
            resource: unknown.clone(),
            amount: 5,
        }],
    );
    assert!(!inventory.resources().contains_key(&unknown));
    assert_eq!(inventory.total_amount(), 0);
}
