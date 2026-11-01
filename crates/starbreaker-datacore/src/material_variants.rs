//! Item tags and tag-selected materials (`SMaterialNodeParams.materialVariants`).
//!
//! Items publish tags into their loadout with `$tag+` / `$tag++` /
//! `$$tag++` tokens in `SAttachableComponentParams.AttachDef.Tags` (the `+`s
//! are the propagation scope) — e.g. a hairstyle publishes `$scalpVHair_36+`.
//! An item's `Components[SGeometryResourceParams].Material` node then picks
//! its material through nested `materialVariants`, each keyed by tags (e.g.
//! `Female` → `scalpVHair_36` → `f_hair_36.mtl`).
//!
//! - [`published_tags`]: the tags a `Tags` string publishes.
//! - [`item_tags`]: an item record's `AttachDef.Tags`.
//! - [`MaterialNode`] / [`material_node`]: the material tree and its
//!   depth-first, first-match resolution against a tag context.
//! - [`entity_material_path`] / [`record_material_path`]: an entity's material,
//!   the material node's or else the geometry's own.

use crate::database::Database;
use crate::query::value::Value;
use crate::types::Record;

/// Tags published by a `Tags` string: `$`-prefixed tokens with a `+` scope
/// suffix, without the markers. Plain `$name` tokens (port/slot names) are not
/// tags.
pub fn published_tags(tags: &str) -> Vec<String> {
    tags.split_whitespace()
        .filter(|t| t.starts_with('$') && t.ends_with('+'))
        .map(|t| t.trim_start_matches('$').trim_end_matches('+').to_owned())
        .filter(|t| !t.is_empty())
        .collect()
}

/// One `SMaterialNodeParams`: a material path selected when all of `tags`
/// (space-separated) are in the context, with nested variants.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MaterialNode {
    pub tags: String,
    pub path: String,
    pub variants: Vec<MaterialNode>,
}

impl MaterialNode {
    fn matches(&self, context: &[String]) -> bool {
        self.tags
            .split_whitespace()
            .all(|tag| context.iter().any(|c| c.eq_ignore_ascii_case(tag)))
    }

    /// Resolve against `context`: descend into the first matching variant;
    /// the deepest non-empty path wins, else this node's own path.
    pub fn resolve(&self, context: &[String]) -> Option<&str> {
        let own = (!self.path.is_empty()).then_some(self.path.as_str());
        self.variants
            .iter()
            .find(|v| v.matches(context))
            .and_then(|v| v.resolve(context))
            .or(own)
    }
}

fn field<'v, 'a>(value: &'v Value<'a>, name: &str) -> Option<&'v Value<'a>> {
    match value {
        Value::Object { fields, .. } => fields.iter().find(|(k, _)| *k == name).map(|(_, v)| v),
        _ => None,
    }
}

fn string(value: &Value, name: &str) -> String {
    match field(value, name) {
        Some(Value::String(s)) => (*s).to_owned(),
        _ => String::new(),
    }
}

fn parse_node(value: &Value) -> MaterialNode {
    let path = field(value, "Material").map(|m| string(m, "path")).unwrap_or_default();
    let variants = match field(value, "materialVariants") {
        Some(Value::Array(items)) => items.iter().map(parse_node).collect(),
        _ => Vec::new(),
    };
    MaterialNode { tags: string(value, "Tags"), path, variants }
}

fn component<'a>(db: &'a Database, record: &Record, path: &str) -> Vec<Value<'a>> {
    db.compile_path::<Value>(record.struct_id(), path)
        .ok()
        .and_then(|compiled| db.query::<Value>(&compiled, record).ok())
        .unwrap_or_default()
}

/// `SAttachableComponentParams.AttachDef.Tags` of an item record.
pub fn item_tags(db: &Database, record: &Record) -> String {
    component(db, record, "Components[SAttachableComponentParams]")
        .iter()
        .filter_map(|c| field(c, "AttachDef"))
        .map(|d| string(d, "Tags"))
        .find(|t| !t.is_empty())
        .unwrap_or_default()
}

/// `Components[SGeometryResourceParams].Material` of an item record.
pub fn material_node(db: &Database, record: &Record) -> Option<MaterialNode> {
    component(db, record, "Components[SGeometryResourceParams]")
        .iter()
        .filter_map(|c| field(c, "Material"))
        .map(parse_node)
        .next()
}

/// An entity's material: its [`MaterialNode`] resolved against `context`,
/// else the geometry's own (`Geometry.Geometry.Material`).
/// Items such as paint editions of backpacks name their material only in the
/// node; without it they fell back to the mesh's MtlName (the base look, or
/// a file that does not exist). Where both name one, the node is the item's
/// own: outlaw_legacy_armor_light_core_01_08_01's geometry still names the
/// base record's `_01_01_01.mtl`, its node `_01_08.mtl`.
pub fn entity_material_path(geometry_material: &str, node: Option<&MaterialNode>, context: &[String]) -> String {
    node.and_then(|n| n.resolve(context))
        .filter(|path| !path.is_empty())
        .unwrap_or(geometry_material)
        .to_owned()
}

/// [`entity_material_path`] for a record: its material node resolved against
/// `context`, else its geometry material.
pub fn record_material_path(db: &Database, record: &Record, geometry_material: &str, context: &[String]) -> String {
    entity_material_path(geometry_material, material_node(db, record).as_ref(), context)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_tags_are_dollar_tokens_with_a_plus_scope() {
        let tags = published_tags("UnifiedHead CustomizerReady  $scalpVHair_36+ $$hairVar_Red++ $Legs++ $slot_a");
        assert_eq!(tags, vec!["scalpVHair_36", "hairVar_Red", "Legs"]);
    }

    fn node(tags: &str, path: &str, variants: Vec<MaterialNode>) -> MaterialNode {
        MaterialNode { tags: tags.into(), path: path.into(), variants }
    }

    #[test]
    fn material_variants_resolve_depth_first_by_tags() {
        let tree = node("", "", vec![
            node("Female", "", vec![
                node("scalpVHair_02", "f_hair_02.mtl", vec![]),
                node("scalpVHair_36", "f_hair_36.mtl", vec![]),
            ]),
            node("Male", "", vec![node("scalpVHair_36", "m_hair_36.mtl", vec![])]),
        ]);
        let ctx = |tags: &[&str]| tags.iter().map(|t| t.to_string()).collect::<Vec<_>>();
        assert_eq!(tree.resolve(&ctx(&["Female", "scalpVHair_36"])), Some("f_hair_36.mtl"));
        assert_eq!(tree.resolve(&ctx(&["male", "SCALPVHAIR_36"])), Some("m_hair_36.mtl"), "case-insensitive");
        assert_eq!(tree.resolve(&ctx(&["Female"])), None, "no variant, no material");
    }

    #[test]
    fn material_node_supplies_an_entity_material_the_geometry_leaves_empty() {
        // Arden-CL Backpack Elysium: SGeometryResourceParams.Geometry names no
        // material; its Material node names mtl_var/rrs_combat_light_backpack_02_01.mtl.
        let node_tree = node("", "backpack/rrs/mtl_var/rrs_combat_light_backpack_02_01.mtl", vec![]);
        assert_eq!(
            entity_material_path("", Some(&node_tree), &[]),
            "backpack/rrs/mtl_var/rrs_combat_light_backpack_02_01.mtl"
        );
        assert_eq!(entity_material_path("", None, &[]), "");
        assert_eq!(entity_material_path("geometry.mtl", None, &[]), "geometry.mtl");
        let empty_node = node("", "", vec![node("Female", "f.mtl", vec![])]);
        assert_eq!(entity_material_path("geometry.mtl", Some(&empty_node), &[]), "geometry.mtl", "a node that resolves to nothing leaves the geometry's");
        let tagged = node("", "", vec![node("Male", "m.mtl", vec![])]);
        assert_eq!(entity_material_path("", Some(&tagged), &["Male".to_string()]), "m.mtl");
    }

    #[test]
    fn material_node_wins_over_the_geometry_material() {
        // outlaw_legacy_armor_light_core_01_08_01: the geometry still names the
        // base record's m_..._01_01_01.mtl, the node its own m_..._01_08.mtl.
        let node_tree = node("", "m_outlaw_legacy_light_core_01_08.mtl", vec![]);
        assert_eq!(
            entity_material_path("m_outlaw_legacy_light_core_01_01_01.mtl", Some(&node_tree), &[]),
            "m_outlaw_legacy_light_core_01_08.mtl"
        );
    }

    #[test]
    fn deeper_material_wins_and_parent_path_is_the_fallback() {
        let tree = node("", "base.mtl", vec![node("A B", "ab.mtl", vec![]), node("A", "", vec![])]);
        let ctx = |tags: &[&str]| tags.iter().map(|t| t.to_string()).collect::<Vec<_>>();
        assert_eq!(tree.resolve(&ctx(&["A", "B"])), Some("ab.mtl"), "all of a variant's tags must match");
        assert_eq!(tree.resolve(&ctx(&["A"])), Some("base.mtl"), "empty variant path falls back to the parent");
        assert_eq!(tree.resolve(&ctx(&[])), Some("base.mtl"));
    }
}
