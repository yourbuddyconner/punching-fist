use kube::{
    api::{Api, PostParams},
    Client,
};
use std::collections::BTreeMap;
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use crate::{
    Task,
    Result,
    OperatorError,
};

pub struct KubeClient {
    client: Client,
    namespace: String,
}

impl KubeClient {
    pub async fn new() -> Result<Self> {
        let client = Client::try_default()
            .await
            .map_err(OperatorError::Kubernetes)?;
        
        let namespace = std::env::var("NAMESPACE").unwrap_or_else(|_| "default".to_string());
        
        Ok(Self {
            client,
            namespace,
        })
    }

    pub async fn create_task_job(&self, task: &Task) -> Result<()> {
        let jobs: Api<k8s_openapi::api::batch::v1::Job> = Api::namespaced(
            self.client.clone(),
            &self.namespace,
        );

        let job = self.create_job_manifest(task);
        jobs.create(&PostParams::default(), &job)
            .await
            .map_err(OperatorError::Kubernetes)?;

        Ok(())
    }

    fn create_job_manifest(&self, task: &Task) -> k8s_openapi::api::batch::v1::Job {
        k8s_openapi::api::batch::v1::Job {
            metadata: k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta {
                name: Some(format!("openhands-task-{}", task.id)),
                namespace: Some(self.namespace.clone()),
                labels: Some({
                    let mut labels = BTreeMap::new();
                    labels.insert("app.kubernetes.io/name".to_string(), "punching-fist".to_string());
                    labels.insert("task.type".to_string(), "openhands".to_string());
                    labels.insert("task.id".to_string(), task.id.clone());
                    labels
                }),
                ..Default::default()
            },
            spec: Some(k8s_openapi::api::batch::v1::JobSpec {
                template: k8s_openapi::api::core::v1::PodTemplateSpec {
                    spec: Some(k8s_openapi::api::core::v1::PodSpec {
                        containers: vec![k8s_openapi::api::core::v1::Container {
                            name: "openhands-task".to_string(),
                            image: Some("docker.all-hands.dev/all-hands-ai/openhands:0.39".to_string()),
                            command: Some(vec!["python".to_string()]),
                            args: Some(vec![
                                "-m".to_string(),
                                "openhands.core.main".to_string(),
                                "-t".to_string(),
                                task.prompt.clone(),
                            ]),
                            env: Some(vec![
                                k8s_openapi::api::core::v1::EnvVar {
                                    name: "LLM_API_KEY".to_string(),
                                    value_from: Some(k8s_openapi::api::core::v1::EnvVarSource {
                                        secret_key_ref: Some(k8s_openapi::api::core::v1::SecretKeySelector {
                                            name: Some("openhands-secrets".to_string()),
                                            key: "api-key".to_string(),
                                            ..Default::default()
                                        }),
                                        ..Default::default()
                                    }),
                                    ..Default::default()
                                },
                                k8s_openapi::api::core::v1::EnvVar {
                                    name: "LLM_MODEL".to_string(),
                                    value: Some(task.model.clone().unwrap_or_else(|| "anthropic/claude-3-7-sonnet-20250219".to_string())),
                                    ..Default::default()
                                },
                                k8s_openapi::api::core::v1::EnvVar {
                                    name: "LOG_ALL_EVENTS".to_string(),
                                    value: Some("true".to_string()),
                                    ..Default::default()
                                },
                                k8s_openapi::api::core::v1::EnvVar {
                                    name: "SANDBOX_RUNTIME_CONTAINER_IMAGE".to_string(),
                                    value: Some("docker.all-hands.dev/all-hands-ai/runtime:0.39-nikolaik".to_string()),
                                    ..Default::default()
                                },
                            ]),
                            volume_mounts: Some(vec![
                                k8s_openapi::api::core::v1::VolumeMount {
                                    name: "docker-sock".to_string(),
                                    mount_path: "/var/run/docker.sock".to_string(),
                                    ..Default::default()
                                },
                                k8s_openapi::api::core::v1::VolumeMount {
                                    name: "openhands-state".to_string(),
                                    mount_path: "/.openhands-state".to_string(),
                                    ..Default::default()
                                },
                            ]),
                            resources: Some(k8s_openapi::api::core::v1::ResourceRequirements {
                                limits: Some({
                                    let mut limits = BTreeMap::new();
                                    limits.insert("cpu".to_string(), Quantity(task.resources.cpu_limit.clone()));
                                    limits.insert("memory".to_string(), Quantity(task.resources.memory_limit.clone()));
                                    limits
                                }),
                                requests: Some({
                                    let mut requests = BTreeMap::new();
                                    requests.insert("cpu".to_string(), Quantity(task.resources.cpu_request.clone()));
                                    requests.insert("memory".to_string(), Quantity(task.resources.memory_request.clone()));
                                    requests
                                }),
                                ..Default::default()
                            }),
                            security_context: Some(k8s_openapi::api::core::v1::SecurityContext {
                                privileged: Some(true),
                                ..Default::default()
                            }),
                            ..Default::default()
                        }],
                        volumes: Some(vec![
                            k8s_openapi::api::core::v1::Volume {
                                name: "docker-sock".to_string(),
                                host_path: Some(k8s_openapi::api::core::v1::HostPathVolumeSource {
                                    path: "/var/run/docker.sock".to_string(),
                                    ..Default::default()
                                }),
                                ..Default::default()
                            },
                            k8s_openapi::api::core::v1::Volume {
                                name: "openhands-state".to_string(),
                                empty_dir: Some(k8s_openapi::api::core::v1::EmptyDirVolumeSource {
                                    ..Default::default()
                                }),
                                ..Default::default()
                            },
                        ]),
                        restart_policy: Some("OnFailure".to_string()),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                backoff_limit: Some(task.max_retries.unwrap_or(3)),
                ttl_seconds_after_finished: Some(3600),
                ..Default::default()
            }),
            ..Default::default()
        }
    }
} 