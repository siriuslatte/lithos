use std::{
    collections::{BTreeMap, HashMap},
    marker::PhantomData,
};

use async_trait::async_trait;
use difference::Changeset;
use serde::Serialize;
use yansi::Paint;

use crate::diagnostics::OperationError;

macro_rules! all_outputs {
    ($expr:expr, $enum:path) => {{
        $expr
            .iter()
            .filter_map(|value| {
                if let $enum(outputs) = value {
                    Some(outputs)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    }};
}
pub(crate) use all_outputs;

macro_rules! single_output {
    ($expr:expr, $enum:path) => {{
        *all_outputs!($expr, $enum)
            .first()
            .expect("Missing expected output")
    }};
}
pub(crate) use single_output;

macro_rules! optional_output {
    ($expr:expr, $enum:path) => {{
        all_outputs!($expr, $enum).first().map(|output| *output)
    }};
}
pub(crate) use optional_output;

pub type ResourceId = String;

pub trait Resource<TInputs, TOutputs>: Clone {
    fn get_id(&self) -> ResourceId;
    fn get_inputs_hash(&self) -> String;
    fn get_outputs_hash(&self) -> String;
    fn get_inputs(&self) -> TInputs;
    fn get_outputs(&self) -> Option<TOutputs>;
    fn get_dependencies(&self) -> Vec<ResourceId>;
    fn set_outputs(&mut self, outputs: TOutputs);
}

#[async_trait]
pub trait ResourceManager<TInputs, TOutputs> {
    async fn get_create_price(
        &self,
        resource_id: &str,
        inputs: TInputs,
        dependency_outputs: Vec<TOutputs>,
    ) -> Result<Option<u32>, OperationError>;

    async fn create(
        &self,
        resource_id: &str,
        inputs: TInputs,
        dependency_outputs: Vec<TOutputs>,
        price: Option<u32>,
    ) -> Result<TOutputs, OperationError>;

    async fn get_update_price(
        &self,
        resource_id: &str,
        inputs: TInputs,
        outputs: TOutputs,
        dependency_outputs: Vec<TOutputs>,
    ) -> Result<Option<u32>, OperationError>;

    async fn update(
        &self,
        resource_id: &str,
        inputs: TInputs,
        outputs: TOutputs,
        dependency_outputs: Vec<TOutputs>,
        price: Option<u32>,
    ) -> Result<TOutputs, OperationError>;

    async fn delete(
        &self,
        resource_id: &str,
        outputs: TOutputs,
        dependency_outputs: Vec<TOutputs>,
    ) -> Result<(), OperationError>;
}

#[async_trait(?Send)]
pub trait EvaluateProgressHandler<TResource, TInputs, TOutputs>
where
    TResource: Resource<TInputs, TOutputs>,
    TInputs: Clone,
    TOutputs: Clone + Serialize,
{
    async fn persist_progress(
        &mut self,
        current_graph: &ResourceGraph<TResource, TInputs, TOutputs>,
        results: &EvaluateResults,
        failures: &[ResourceFailure],
    ) -> Result<(), String>;
}

#[derive(Debug, Default, Clone)]
pub struct EvaluateResults {
    pub created_count: u32,
    pub updated_count: u32,
    pub deleted_count: u32,
    pub noop_count: u32,
    pub skipped_count: u32,
}

enum OperationResult<TOutputs> {
    Skipped(String),
    Noop,
    Failed(OperationError),
    SucceededDelete,
    SucceededCreate(TOutputs),
    SucceededUpdate(TOutputs),
}

#[derive(Debug, Clone)]
pub struct ResourceFailure {
    pub resource_id: ResourceId,
    pub error: OperationError,
}

#[derive(Debug, Clone)]
pub struct EvaluateError {
    pub results: EvaluateResults,
    pub failures: Vec<ResourceFailure>,
}

enum ProgressEvent {
    None,
    Mutation,
}

impl EvaluateError {
    pub fn failure_count(&self) -> usize {
        self.failures.len()
    }

    pub fn applied_mutation_count(&self) -> u32 {
        self.results.created_count + self.results.updated_count + self.results.deleted_count
    }
}

impl std::fmt::Display for EvaluateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Failed {} change(s) while evaluating the resource graph.",
            self.failures.len()
        )
    }
}

impl std::error::Error for EvaluateError {}

fn get_changeset(previous_hash: &str, new_hash: &str) -> Changeset {
    Changeset::new(previous_hash, new_hash, "\n")
}

pub struct ResourceGraph<TResource, TInputs, TOutputs>
where
    TResource: Resource<TInputs, TOutputs>,
    TInputs: Clone,
    TOutputs: Clone,
{
    phantom_inputs: std::marker::PhantomData<TInputs>,
    phantom_outputs: std::marker::PhantomData<TOutputs>,
    resources: HashMap<ResourceId, TResource>,
}

impl<TResource, TInputs, TOutputs> ResourceGraph<TResource, TInputs, TOutputs>
where
    TResource: Resource<TInputs, TOutputs>,
    TInputs: Clone,
    TOutputs: Clone,
    TOutputs: Serialize,
{
    pub fn new(resources: &[TResource]) -> Self {
        Self {
            resources: resources
                .iter()
                .map(|resource| (resource.get_id(), resource.clone()))
                .collect(),
            phantom_inputs: PhantomData,
            phantom_outputs: PhantomData,
        }
    }

    pub fn get_outputs(&self, resource_id: &str) -> Option<TOutputs> {
        self.resources
            .get(resource_id)
            .and_then(|resource| resource.get_outputs())
    }

    fn get_dependency_graph(&self) -> BTreeMap<ResourceId, Vec<ResourceId>> {
        self.resources
            .iter()
            .map(|(id, resource)| (id.clone(), resource.get_dependencies()))
            .collect()
    }

    fn get_topological_order(&self) -> Result<Vec<ResourceId>, String> {
        let mut dependency_graph = self.get_dependency_graph();

        let mut start_nodes: Vec<ResourceId> = dependency_graph
            .iter()
            .filter_map(|(node, deps)| {
                if deps.is_empty() {
                    Some(node.clone())
                } else {
                    None
                }
            })
            .collect();

        let mut ordered: Vec<ResourceId> = Vec::new();
        while let Some(start_node) = start_nodes.pop() {
            ordered.push(start_node.clone());
            for (node, deps) in dependency_graph.iter_mut() {
                if deps.contains(&start_node) {
                    deps.retain(|dep| dep != &start_node);
                    if deps.is_empty() {
                        start_nodes.push(node.clone());
                    }
                }
            }
        }

        let has_cycles = dependency_graph.iter().any(|(_, deps)| !deps.is_empty());
        match has_cycles {
            true => Err("Cannot evaluate resource graph because it has cycles".to_owned()),
            false => Ok(ordered),
        }
    }

    pub fn get_resource_list(&self) -> Vec<TResource> {
        self.get_topological_order()
            .unwrap()
            .iter()
            .map(|id| self.resources.get(id).unwrap().clone())
            .collect()
    }

    fn get_dependency_outputs(&self, resource: &TResource) -> Option<Vec<TOutputs>> {
        let mut dependency_outputs: Vec<TOutputs> = Vec::new();
        for dependency in resource.get_dependencies() {
            let resource = self.resources.get(&dependency);
            if let Some(resource) = resource {
                if let Some(outputs) = resource.get_outputs() {
                    dependency_outputs.push(outputs);
                } else {
                    return None;
                }
            } else {
                return None;
            }
        }
        Some(dependency_outputs)
    }

    fn get_dependency_outputs_hash(&self, dependency_outputs: Vec<TOutputs>) -> String {
        // TODO: Should we separate hashes from displays?
        let hash = serde_yaml::to_string(&dependency_outputs)
            .map_err(|e| format!("Failed to compute dependency outputs hash\n\t{}", e))
            .unwrap();
        if hash.is_empty() {
            ""
        } else {
            // We remove first 4 characters to remove "---\n", and we trim the end to remove "\n"
            hash[4..].trim_end()
        }
        .to_owned()
    }

    fn handle_operation_result(
        &mut self,
        results: &mut EvaluateResults,
        failures_count: &mut u32,
        failures: &mut Vec<ResourceFailure>,
        previous_graph: &ResourceGraph<TResource, TInputs, TOutputs>,
        resource_id: &str,
        operation_result: OperationResult<TOutputs>,
    ) -> ProgressEvent {
        // TODO: Improve DRY here
        match operation_result {
            OperationResult::SucceededDelete => {
                // No need to update the graph since it's already not present
                results.deleted_count += 1;
                let previous_resource = previous_graph.resources.get(resource_id).unwrap();
                logger::end_action_with_results(
                    "Succeeded with outputs:",
                    get_changeset(&previous_resource.get_outputs_hash(), ""),
                );
                ProgressEvent::Mutation
            }
            OperationResult::SucceededCreate(outputs) => {
                // Update the resource with the new outputs
                let resource = self.resources.get_mut(resource_id).unwrap();
                resource.set_outputs(outputs);

                results.created_count += 1;
                logger::end_action_with_results(
                    "Succeeded with outputs:",
                    get_changeset("", &resource.get_outputs_hash()),
                );
                ProgressEvent::Mutation
            }
            OperationResult::SucceededUpdate(outputs) => {
                // Update the resource with the new outputs
                let resource = self.resources.get_mut(resource_id).unwrap();
                resource.set_outputs(outputs);

                results.updated_count += 1;
                let previous_resource = previous_graph.resources.get(resource_id).unwrap();
                logger::end_action_with_results(
                    "Succeeded with outputs:",
                    get_changeset(
                        &previous_resource.get_outputs_hash(),
                        &resource.get_outputs_hash(),
                    ),
                );
                ProgressEvent::Mutation
            }
            OperationResult::Noop => {
                // There was no need to create or update the resource. We will update the resource
                // with the previous outputs
                let previous_resource = previous_graph.resources.get(resource_id).unwrap();
                let resource = self.resources.get_mut(resource_id).unwrap();
                resource.set_outputs(
                    previous_resource
                        .get_outputs()
                        .expect("Existing resource should have outputs."),
                );

                results.noop_count += 1;
                ProgressEvent::None
            }
            OperationResult::Skipped(reason) => {
                // The resource was not evaluated. If the resource existed previously, we will copy
                // the old version into this graph. Otherwise, we will remove this resource from the
                // graph.
                if let Some(previous_resource) = previous_graph.resources.get(resource_id) {
                    self.resources
                        .insert(resource_id.to_owned(), previous_resource.to_owned());
                } else {
                    self.resources.remove(resource_id);
                }

                results.skipped_count += 1;
                logger::end_action(format!("Skipped: {}", Paint::yellow(reason)));
                ProgressEvent::None
            }
            OperationResult::Failed(error) => {
                // An error occurred while creating or updating the resource. If the
                // resource existed previously, we will copy the old version into this
                // graph. Otherwise, we will remove this resource from the graph.
                if let Some(previous_resource) = previous_graph.resources.get(resource_id) {
                    self.resources
                        .insert(resource_id.to_owned(), previous_resource.to_owned());
                } else {
                    self.resources.remove(resource_id);
                }

                *failures_count += 1;
                failures.push(ResourceFailure {
                    resource_id: resource_id.to_owned(),
                    error: error.clone(),
                });
                for diagnostic in error.diagnostics() {
                    if let Some(detail) = &diagnostic.detail {
                        logger::log(format!("  {}", detail));
                    }
                    for cause in &diagnostic.probable_causes {
                        logger::log(format!("  likely: {}", cause));
                    }
                    for next_step in &diagnostic.next_steps {
                        logger::log(format!("  next: {}", next_step));
                    }
                }
                logger::end_action(format!("Failed: {}", Paint::red(error.summary())));
                ProgressEvent::None
            }
        }
    }

    async fn persist_evaluate_progress(
        &self,
        progress: &mut dyn EvaluateProgressHandler<TResource, TInputs, TOutputs>,
        results: &EvaluateResults,
        failures: &[ResourceFailure],
        resource_id: &str,
    ) -> Result<(), EvaluateError> {
        progress
            .persist_progress(self, results, failures)
            .await
            .map_err(|error| EvaluateError {
                results: results.clone(),
                failures: vec![ResourceFailure {
                    resource_id: resource_id.to_owned(),
                    error: OperationError::new(
                        format!(
                            "Failed to persist deployment progress after applying {}\n\t{}",
                            resource_id, error
                        ),
                        Vec::new(),
                    ),
                }],
            })
    }

    async fn evaluate_delete<TManager>(
        &self,
        previous_graph: &ResourceGraph<TResource, TInputs, TOutputs>,
        manager: &mut TManager,
        resource_id: &str,
    ) -> OperationResult<TOutputs>
    where
        TManager: ResourceManager<TInputs, TOutputs>,
    {
        let resource = previous_graph.resources.get(resource_id).unwrap();
        let dependency_outputs = previous_graph
            .get_dependency_outputs(resource)
            .expect("Previous graph should be complete.");

        let inputs_hash = resource.get_inputs_hash();
        let dependencies_hash = self.get_dependency_outputs_hash(dependency_outputs.clone());
        logger::start_action(format!(
            "{} Deleting: {}",
            Paint::red("-"),
            resource.get_id()
        ));
        logger::log("Dependencies:");
        logger::log_changeset(get_changeset(&dependencies_hash, &dependencies_hash));
        logger::log("Inputs:");
        logger::log_changeset(get_changeset(&inputs_hash, ""));

        match manager
            .delete(
                resource_id,
                resource
                    .get_outputs()
                    .expect("Existing resource should have outputs."),
                dependency_outputs,
            )
            .await
        {
            Ok(()) => OperationResult::SucceededDelete,
            Err(error) => OperationResult::Failed(error),
        }
    }

    async fn evaluate_create_or_update<TManager>(
        &self,
        previous_graph: &ResourceGraph<TResource, TInputs, TOutputs>,
        manager: &mut TManager,
        resource_id: &str,
        allow_purchases: bool,
    ) -> OperationResult<TOutputs>
    where
        TManager: ResourceManager<TInputs, TOutputs>,
    {
        let resource = self.resources.get(resource_id).unwrap();
        let inputs_hash = resource.get_inputs_hash();
        let dependency_outputs = self.get_dependency_outputs(resource);

        let previous_resource = previous_graph.resources.get(resource_id);

        if let Some(previous_resource) = previous_resource {
            // Check for changes
            let previous_hash = previous_resource.get_inputs_hash();
            let previous_dependency_outputs = previous_graph
                .get_dependency_outputs(previous_resource)
                .expect("Previous graph should be complete.");
            let previous_dependencies_hash =
                self.get_dependency_outputs_hash(previous_dependency_outputs);

            // TODO: How can we determine between update/noop?
            let dependency_outputs = match dependency_outputs {
                Some(v) => v,
                None => {
                    logger::start_action(format!(
                        "{} Update or Noop: {}",
                        Paint::new("○").dimmed(),
                        resource.get_id(),
                    ));
                    return OperationResult::Skipped(
                        "A dependency failed to produce outputs.".to_owned(),
                    );
                }
            };
            let dependencies_hash = self.get_dependency_outputs_hash(dependency_outputs.clone());

            if previous_hash == inputs_hash && previous_dependencies_hash == dependencies_hash {
                // No changes
                return OperationResult::Noop;
            }

            // This resource has changed
            logger::start_action(format!("{} Updating: {}", Paint::yellow("~"), resource_id));
            logger::log("Dependencies:");
            logger::log_changeset(get_changeset(
                &previous_dependencies_hash,
                &dependencies_hash,
            ));
            logger::log("Inputs:");
            logger::log_changeset(get_changeset(&previous_hash, &inputs_hash));

            let outputs = previous_resource
                .get_outputs()
                .expect("Existing resource should have outputs.");

            let price = match manager
                .get_update_price(
                    resource_id,
                    resource.get_inputs(),
                    outputs.clone(),
                    dependency_outputs.clone(),
                )
                .await
            {
                Ok(Some(price)) if price > 0 => {
                    if allow_purchases {
                        logger::log("");
                        logger::log(Paint::yellow(format!(
                            "{} Robux will be charged from your account.",
                            price
                        )));
                        Some(price)
                    } else {
                        return OperationResult::Skipped(format!(
                                "Resource would cost {} Robux to create. Give Mantle permission to make purchases with --allow-purchases.",
                                price
                            ));
                    }
                }
                Err(error) => return OperationResult::Failed(error),
                Ok(_) => None,
            };

            match manager
                .update(
                    resource_id,
                    resource.get_inputs(),
                    outputs,
                    dependency_outputs,
                    price,
                )
                .await
            {
                Ok(outputs) => OperationResult::SucceededUpdate(outputs),
                Err(error) => OperationResult::Failed(error),
            }
        } else {
            // Create
            logger::start_action(format!("{} Creating: {}", Paint::green("+"), resource_id));

            let dependency_outputs = match dependency_outputs {
                Some(v) => v,
                None => {
                    return OperationResult::Skipped(
                        "A dependency failed to produce outputs.".to_owned(),
                    );
                }
            };
            let dependencies_hash = self.get_dependency_outputs_hash(dependency_outputs.clone());

            logger::log("Dependencies:");
            logger::log_changeset(get_changeset(&dependencies_hash, &dependencies_hash));
            logger::log("Inputs:");
            logger::log_changeset(get_changeset("", &inputs_hash));

            let price = match manager
                .get_create_price(
                    resource_id,
                    resource.get_inputs(),
                    dependency_outputs.clone(),
                )
                .await
            {
                Ok(Some(price)) if price > 0 => {
                    if allow_purchases {
                        logger::log("");
                        logger::log(Paint::yellow(format!(
                            "{} Robux will be charged from your account.",
                            price
                        )));
                        Some(price)
                    } else {
                        return OperationResult::Skipped(format!(
                                "Resource would cost {} Robux to create. Give Mantle permission to make purchases with --allow-purchases.",
                                price
                            ));
                    }
                }
                Err(error) => return OperationResult::Failed(error),
                Ok(_) => None,
            };

            match manager
                .create(
                    resource_id,
                    resource.get_inputs(),
                    dependency_outputs,
                    price,
                )
                .await
            {
                Ok(outputs) => OperationResult::SucceededCreate(outputs),
                Err(error) => OperationResult::Failed(error),
            }
        }
    }

    pub async fn evaluate<TManager>(
        &mut self,
        previous_graph: &ResourceGraph<TResource, TInputs, TOutputs>,
        manager: &mut TManager,
        allow_purchases: bool,
    ) -> Result<EvaluateResults, EvaluateError>
    where
        TManager: ResourceManager<TInputs, TOutputs>,
    {
        self.evaluate_with_progress(previous_graph, manager, allow_purchases, None)
            .await
    }

    pub async fn evaluate_with_progress<TManager>(
        &mut self,
        previous_graph: &ResourceGraph<TResource, TInputs, TOutputs>,
        manager: &mut TManager,
        allow_purchases: bool,
        mut progress: Option<&mut dyn EvaluateProgressHandler<TResource, TInputs, TOutputs>>,
    ) -> Result<EvaluateResults, EvaluateError>
    where
        TManager: ResourceManager<TInputs, TOutputs>,
    {
        let mut results = EvaluateResults::default();
        let mut failures_count: u32 = 0;
        let mut failures: Vec<ResourceFailure> = Vec::new();

        // Iterate over previous resources in reverse order so that leaf resources are removed first
        let mut previous_resource_order =
            previous_graph
                .get_topological_order()
                .map_err(|error| EvaluateError {
                    results: results.clone(),
                    failures: vec![ResourceFailure {
                        resource_id: "resource-graph".to_owned(),
                        error: OperationError::new(error.clone(), Vec::new()),
                    }],
                })?;
        previous_resource_order.reverse();
        for resource_id in previous_resource_order.iter() {
            if self.resources.contains_key(resource_id) {
                continue;
            }

            let operation_result: OperationResult<TOutputs> = self
                .evaluate_delete(previous_graph, manager, resource_id)
                .await;
            let progress_event = self.handle_operation_result(
                &mut results,
                &mut failures_count,
                &mut failures,
                previous_graph,
                resource_id,
                operation_result,
            );
            if matches!(progress_event, ProgressEvent::Mutation) {
                if let Some(progress_handler) = progress.as_mut() {
                    self.persist_evaluate_progress(
                        *progress_handler,
                        &results,
                        &failures,
                        resource_id,
                    )
                    .await?;
                }
            }
        }

        let resource_order = self
            .get_topological_order()
            .map_err(|error| EvaluateError {
                results: results.clone(),
                failures: vec![ResourceFailure {
                    resource_id: "resource-graph".to_owned(),
                    error: OperationError::new(error.clone(), Vec::new()),
                }],
            })?;
        for resource_id in resource_order.iter() {
            let operation_result = self
                .evaluate_create_or_update(previous_graph, manager, resource_id, allow_purchases)
                .await;
            let progress_event = self.handle_operation_result(
                &mut results,
                &mut failures_count,
                &mut failures,
                previous_graph,
                resource_id,
                operation_result,
            );
            if matches!(progress_event, ProgressEvent::Mutation) {
                if let Some(progress_handler) = progress.as_mut() {
                    self.persist_evaluate_progress(
                        *progress_handler,
                        &results,
                        &failures,
                        resource_id,
                    )
                    .await?;
                }
            }
        }

        if failures_count > 0 {
            Err(EvaluateError { results, failures })
        } else {
            Ok(results)
        }
    }

    pub fn diff(
        &self,
        previous_graph: &ResourceGraph<TResource, TInputs, TOutputs>,
    ) -> Result<ResourceGraphDiff, String> {
        let mut diff = ResourceGraphDiff {
            removals: BTreeMap::new(),
            additions: BTreeMap::new(),
            changes: BTreeMap::new(),
            dependency_changes: BTreeMap::new(),
        };

        // Iterate over previous resources in reverse order so that leaf resources are removed first
        let mut previous_resource_order = previous_graph.get_topological_order()?;
        previous_resource_order.reverse();

        for resource_id in previous_resource_order.iter() {
            if self.resources.contains_key(resource_id) {
                continue;
            }

            diff.removals.insert(
                resource_id.to_owned(),
                ResourceRemoval {
                    previous_inputs_hash: previous_graph
                        .resources
                        .get(resource_id)
                        .unwrap()
                        .get_inputs_hash(),
                    previous_outputs_hash: previous_graph
                        .resources
                        .get(resource_id)
                        .unwrap()
                        .get_outputs_hash(),
                },
            );
        }

        let resource_order = self.get_topological_order()?;
        for resource_id in resource_order.iter() {
            let resource = self.resources.get(resource_id).unwrap();
            let inputs_hash = resource.get_inputs_hash();

            let previous_resource = previous_graph.resources.get(resource_id);

            if let Some(previous_resource) = previous_resource {
                let previous_hash = previous_resource.get_inputs_hash();
                if previous_hash != inputs_hash {
                    diff.changes.insert(
                        resource_id.to_owned(),
                        ResourceChange {
                            previous_inputs_hash: previous_hash,
                            previous_outputs_hash: previous_resource.get_outputs_hash(),
                            current_inputs_hash: inputs_hash,
                        },
                    );
                } else {
                    let dependencies = resource.get_dependencies();
                    #[allow(clippy::iter_overeager_cloned)]
                    let changed_dependencies: Vec<_> = dependencies
                        .iter()
                        .cloned()
                        .filter(|x| diff.additions.contains_key(x) || diff.changes.contains_key(x))
                        .collect();

                    if !changed_dependencies.is_empty() {
                        diff.dependency_changes.insert(
                            resource_id.to_owned(),
                            ResourceDependencyChange {
                                previous_inputs_hash: previous_hash,
                                previous_outputs_hash: previous_resource.get_outputs_hash(),
                                current_inputs_hash: inputs_hash,
                                changed_dependencies,
                            },
                        );
                    }
                }
            } else {
                diff.additions.insert(
                    resource_id.to_owned(),
                    ResourceAddition {
                        current_inputs_hash: inputs_hash,
                    },
                );
            }
        }

        Ok(diff)
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use super::*;

    #[derive(Clone)]
    struct TestResource {
        id: String,
        inputs: String,
        outputs: Option<String>,
        dependencies: Vec<ResourceId>,
    }

    impl TestResource {
        fn new(id: &str) -> Self {
            Self {
                id: id.to_owned(),
                inputs: id.to_owned(),
                outputs: None,
                dependencies: Vec::new(),
            }
        }
    }

    impl Resource<String, String> for TestResource {
        fn get_id(&self) -> ResourceId {
            self.id.clone()
        }

        fn get_inputs_hash(&self) -> String {
            self.inputs.clone()
        }

        fn get_outputs_hash(&self) -> String {
            self.outputs.clone().unwrap_or_default()
        }

        fn get_inputs(&self) -> String {
            self.inputs.clone()
        }

        fn get_outputs(&self) -> Option<String> {
            self.outputs.clone()
        }

        fn get_dependencies(&self) -> Vec<ResourceId> {
            self.dependencies.clone()
        }

        fn set_outputs(&mut self, outputs: String) {
            self.outputs = Some(outputs);
        }
    }

    struct TestManager;

    #[async_trait]
    impl ResourceManager<String, String> for TestManager {
        async fn get_create_price(
            &self,
            _resource_id: &str,
            _inputs: String,
            _dependency_outputs: Vec<String>,
        ) -> Result<Option<u32>, OperationError> {
            Ok(None)
        }

        async fn create(
            &self,
            resource_id: &str,
            _inputs: String,
            _dependency_outputs: Vec<String>,
            _price: Option<u32>,
        ) -> Result<String, OperationError> {
            Ok(format!("created:{}", resource_id))
        }

        async fn get_update_price(
            &self,
            _resource_id: &str,
            _inputs: String,
            _outputs: String,
            _dependency_outputs: Vec<String>,
        ) -> Result<Option<u32>, OperationError> {
            Ok(None)
        }

        async fn update(
            &self,
            resource_id: &str,
            _inputs: String,
            _outputs: String,
            _dependency_outputs: Vec<String>,
            _price: Option<u32>,
        ) -> Result<String, OperationError> {
            Ok(format!("updated:{}", resource_id))
        }

        async fn delete(
            &self,
            _resource_id: &str,
            _outputs: String,
            _dependency_outputs: Vec<String>,
        ) -> Result<(), OperationError> {
            Ok(())
        }
    }

    #[derive(Default)]
    struct RecordingProgress {
        applied_counts: Vec<u32>,
    }

    #[async_trait(?Send)]
    impl EvaluateProgressHandler<TestResource, String, String> for RecordingProgress {
        async fn persist_progress(
            &mut self,
            _current_graph: &ResourceGraph<TestResource, String, String>,
            results: &EvaluateResults,
            _failures: &[ResourceFailure],
        ) -> Result<(), String> {
            self.applied_counts
                .push(results.created_count + results.updated_count + results.deleted_count);
            Ok(())
        }
    }

    struct FailingProgress {
        calls: u32,
    }

    #[async_trait(?Send)]
    impl EvaluateProgressHandler<TestResource, String, String> for FailingProgress {
        async fn persist_progress(
            &mut self,
            _current_graph: &ResourceGraph<TestResource, String, String>,
            _results: &EvaluateResults,
            _failures: &[ResourceFailure],
        ) -> Result<(), String> {
            self.calls += 1;
            Err("disk full".to_owned())
        }
    }

    #[tokio::test]
    async fn evaluate_with_progress_persists_after_each_mutation() {
        let previous_graph = ResourceGraph::new(&[] as &[TestResource]);
        let mut next_graph =
            ResourceGraph::new(&[TestResource::new("alpha"), TestResource::new("beta")]);
        let mut manager = TestManager;
        let mut progress = RecordingProgress::default();

        let results = next_graph
            .evaluate_with_progress(&previous_graph, &mut manager, false, Some(&mut progress))
            .await
            .unwrap();

        assert_eq!(results.created_count, 2);
        assert_eq!(progress.applied_counts, vec![1, 2]);
    }

    #[tokio::test]
    async fn evaluate_with_progress_returns_error_when_persistence_fails() {
        let previous_graph = ResourceGraph::new(&[] as &[TestResource]);
        let mut next_graph = ResourceGraph::new(&[TestResource::new("alpha")]);
        let mut manager = TestManager;
        let mut progress = FailingProgress { calls: 0 };

        let error = next_graph
            .evaluate_with_progress(&previous_graph, &mut manager, false, Some(&mut progress))
            .await
            .unwrap_err();

        assert_eq!(error.applied_mutation_count(), 1);
        assert_eq!(error.failure_count(), 1);
        assert_eq!(error.failures[0].resource_id, "alpha");
        assert!(error.failures[0]
            .error
            .summary()
            .contains("Failed to persist deployment progress after applying alpha"));
    }
}

#[derive(Serialize)]
pub struct ResourceGraphDiff {
    pub removals: BTreeMap<ResourceId, ResourceRemoval>,
    pub additions: BTreeMap<ResourceId, ResourceAddition>,
    pub changes: BTreeMap<ResourceId, ResourceChange>,
    pub dependency_changes: BTreeMap<ResourceId, ResourceDependencyChange>,
}

#[derive(Serialize)]
pub struct ResourceRemoval {
    pub previous_inputs_hash: String,
    pub previous_outputs_hash: String,
}

#[derive(Serialize)]
pub struct ResourceAddition {
    pub current_inputs_hash: String,
}

#[derive(Serialize)]
pub struct ResourceChange {
    pub previous_inputs_hash: String,
    pub previous_outputs_hash: String,
    pub current_inputs_hash: String,
}

#[derive(Serialize)]
pub struct ResourceDependencyChange {
    pub previous_inputs_hash: String,
    pub previous_outputs_hash: String,
    pub current_inputs_hash: String,
    pub changed_dependencies: Vec<ResourceId>,
}
