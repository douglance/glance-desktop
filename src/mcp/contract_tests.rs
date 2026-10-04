//! Check the published contract against Serde's actual action inventory.
use super::*;
use crate::editor::actions::Action;
use serde::{Deserialize, de};
use std::collections::BTreeSet;

#[derive(Debug)]
struct InventoryError(Vec<&'static str>);
impl std::fmt::Display for InventoryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "action inventory: {:?}", self.0)
    }
}
impl std::error::Error for InventoryError {}
impl de::Error for InventoryError {
    fn custom<T: std::fmt::Display>(message: T) -> Self {
        panic!("Unexpected inventory deserialization error: {message}")
    }
    fn unknown_variant(_: &str, expected: &'static [&'static str]) -> Self {
        Self(expected.to_vec())
    }
}

fn action_schema() -> Value {
    tools()
        .into_iter()
        .find(|t| t["name"] == "dispatch_action")
        .unwrap()["inputSchema"]["properties"]["action"]
        .clone()
}

#[test]
fn every_serializable_action_is_exposed_or_explicitly_excluded() {
    // Serde supplies this list, including future variants and wire renames.
    // No source parsing or second handwritten list of application actions.
    let deserializer = de::value::MapDeserializer::<_, InventoryError>::new(
        [("type", "__mcp_inventory__")].into_iter(),
    );
    let Err(InventoryError(variants)) = Action::deserialize(deserializer) else {
        panic!("Inventory sentinel unexpectedly became a valid action")
    };
    let schema = action_schema();
    let exposed: BTreeSet<_> = schema["properties"]["type"]["enum"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap())
        .collect();
    // These have semantic MCP alternatives. Prepared documents are serde(skip)
    // and must never appear in either the wire inventory or published schema.
    let excluded = [
        (
            "edit",
            "revision-scoped document tools instead of raw indices",
        ),
        (
            "begin_backdrop_adjustment",
            "set_backdrop_control instead of pointer gestures",
        ),
        (
            "begin_animation_adjustment",
            "set_animation_control instead of pointer gestures",
        ),
    ];
    for (name, reason) in excluded {
        assert!(!exposed.contains(name), "{name}: {reason}");
    }
    let accounted: BTreeSet<_> = exposed
        .into_iter()
        .chain(excluded.map(|(name, _)| name))
        .collect();
    assert!(!accounted.contains("apply_prepared_document"));
    assert_eq!(
        accounted,
        variants.into_iter().collect(),
        "Update the MCP schema or document an intentional exclusion for each action"
    );
}

#[test]
fn every_exposed_action_has_a_valid_round_trip_payload() {
    let fixtures = [
        json!({"type":"capture","area":true}),
        json!({"type":"open_path","path":"/tmp/glance-synthetic.png"}),
        json!({"type":"select_tool","tool":"arrow"}),
        json!({"type":"set_color","color":[10,20,30,128]}),
        json!({"type":"set_stroke_width","width":3}),
        json!({"type":"set_appearance","style":crate::style::Style::default()}),
        json!({"type":"set_magnifier_zoom","zoom":3}),
        json!({"type":"set_counter_number","number":7}),
        json!({"type":"set_crop_ratio","ratio":null}),
        json!({"type":"nudge_selection","delta":[1,2],"remember":true}),
        json!({"type":"zoom","factor":2}),
        json!({"type":"zoom_at","factor":2,"anchor":[10,20]}),
        json!({"type":"pan_by","delta":[10,20]}),
        json!({"type":"close_panel","panel":"animation"}),
        json!({"type":"set_resize_scale","scale":2}),
        json!({"type":"resize","scale":2,"smart":true}),
        json!({"type":"set_backdrop","backdrop":crate::backdrop::Backdrop::default()}),
        json!({"type":"set_backdrop_format","format":"shorts"}),
        json!({"type":"set_backdrop_fill","gradient":true}),
        json!({"type":"select_motion","motion":"aurora"}),
        json!({"type":"set_backdrop_preset","preset":0}),
        json!({"type":"set_backdrop_control","control":"inside_padding","value":20}),
        json!({"type":"select_entrance","effect":"diagonal"}),
        json!({"type":"set_image_animation","animation":crate::animation::ImageAnimation::default()}),
        json!({"type":"set_animation_control","control":"duration","value":800}),
        json!({"type":"seek_animation","seconds":1.5}),
        json!({"type":"export_animation","format":"gif"}),
    ];
    let schema = action_schema();
    for name in schema["properties"]["type"]["enum"].as_array().unwrap() {
        let payload = fixtures
            .iter()
            .find(|f| f["type"] == *name)
            .cloned()
            .unwrap_or_else(|| json!({"type":name}));
        let action = Action::from_json(payload.clone())
            .unwrap_or_else(|e| panic!("Add a complete fixture for {name}: {e}"));
        validate_tool("dispatch_action", &json!({"action":payload})).unwrap();
        // Serializing fills defaulted nested fields and exposes any schema
        // field omissions even when a short fixture happens to deserialize.
        let canonical = serde_json::to_value(action).unwrap();
        validate_tool("dispatch_action", &json!({"action":canonical})).unwrap();
        Action::from_json(canonical).unwrap();
    }
    for format in crate::backdrop::Format::ALL {
        validate_tool(
            "dispatch_action",
            &json!({"action":{"type":"set_backdrop_format","format":format}}),
        )
        .unwrap();
    }
}
