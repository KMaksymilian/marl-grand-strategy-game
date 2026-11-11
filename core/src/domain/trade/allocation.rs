use crate::{
    economy::{
        gold_inventory::GoldInventory, inventory::ResourceInventory, resources::ResourceType,
    },
    trade::book::TradeBook,
};
use std::collections::HashMap;

/// TRADE ALLOCATION ENGINE
pub struct TradeAllocationEngine;
impl TradeAllocationEngine {
    pub fn execute(
        book: &mut TradeBook,
        gold_inventory: &mut GoldInventory,
        inventory: &mut ResourceInventory,
        demands: (HashMap<ResourceType, u32>, u32),
    ) {
        let (resource_demands, gold_demand) = demands;

        for (res_type, amount) in resource_demands {
            if !inventory.consume(res_type, amount) {
                let avaiable_units: u32 = inventory.consume_available(res_type);
                book.update_sell_resource_amount(res_type, avaiable_units);
            }
        }

        Self::execute_gold_allocation(book, gold_inventory, gold_demand);
    }

    fn execute_gold_allocation(
        book: &mut TradeBook,
        gold_inventory: &mut GoldInventory,
        gold_demand: u32,
    ) {
        if !gold_inventory.consume(gold_demand) {
            let gold_amount: u32 = gold_inventory.consume_available();
            book.add_gold_amount(gold_amount);
        } else {
            book.add_gold_amount(gold_demand);
        }
    }

    pub fn execute_remained_resources(
        gold_inventory: &mut GoldInventory,
        inventory: &mut ResourceInventory,
        returned: (HashMap<ResourceType, u32>, u32),
    ) {
        let (returned_resources, returned_gold) = returned;
        gold_inventory.add(returned_gold);
        for (res_type, amount) in returned_resources {
            inventory.add(res_type, amount); // result ignored ???
        }
    }
}
