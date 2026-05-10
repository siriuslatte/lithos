//! [`RobloxResource`] – the concrete [`Resource`] implementation that pairs
//! [`RobloxInputs`] with [`RobloxOutputs`] for the resource graph.

use serde::{Deserialize, Serialize};

use crate::resource_graph::{Resource, ResourceId};

use super::{inputs::RobloxInputs, outputs::RobloxOutputs};

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RobloxResource {
    id: ResourceId,
    inputs: RobloxInputs,
    outputs: Option<RobloxOutputs>,
    dependencies: Vec<ResourceId>,
}

impl RobloxResource {
    pub fn new(id: &str, inputs: RobloxInputs, dependencies: &[&RobloxResource]) -> Self {
        Self {
            id: id.to_owned(),
            inputs,
            outputs: None,
            dependencies: dependencies.iter().map(|d| d.get_id()).collect(),
        }
    }

    pub fn existing(
        id: &str,
        inputs: RobloxInputs,
        outputs: RobloxOutputs,
        dependencies: &[&RobloxResource],
    ) -> Self {
        Self {
            id: id.to_owned(),
            inputs,
            outputs: Some(outputs),
            dependencies: dependencies.iter().map(|d| d.get_id()).collect(),
        }
    }

    pub fn add_dependency(&mut self, dependency: &RobloxResource) -> &mut Self {
        self.dependencies.push(dependency.get_id());
        self
    }
}

impl Resource<RobloxInputs, RobloxOutputs> for RobloxResource {
    fn get_id(&self) -> String {
        self.id.clone()
    }

    fn get_inputs_hash(&self) -> String {
        // TODO: Should we separate hashes from displays?
        let hash = serde_yaml::to_string(&self.inputs)
            .map_err(|e| format!("Failed to compute inputs hash\n\t{}", e))
            .unwrap();
        if hash.is_empty() {
            ""
        } else {
            // We remove first 4 characters to remove "---\n", and we trim the end to remove "\n"
            hash[4..].trim_end()
        }
        .to_owned()
    }

    fn get_outputs_hash(&self) -> String {
        // TODO: Should we separate hashes from displays?
        let hash = serde_yaml::to_string(&self.outputs)
            .map_err(|e| format!("Failed to compute outputs hash\n\t{}", e))
            .unwrap();
        if hash.is_empty() {
            ""
        } else {
            // We remove first 4 characters to remove "---\n", and we trim the end to remove "\n"
            hash[4..].trim_end()
        }
        .to_owned()
    }

    fn get_inputs(&self) -> RobloxInputs {
        self.inputs.clone()
    }

    fn get_outputs(&self) -> Option<RobloxOutputs> {
        self.outputs.clone()
    }

    fn get_dependencies(&self) -> Vec<ResourceId> {
        self.dependencies.clone()
    }

    fn set_outputs(&mut self, outputs: RobloxOutputs) {
        self.outputs = Some(outputs);
    }
}
