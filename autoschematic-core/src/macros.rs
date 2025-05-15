#[macro_export]
macro_rules! get_resource_output {
    ($resource:expr) => {{
        Ok(Some(GetResourceOutput {
            resource_definition: Resource::to_string(&$resource)?,
            outputs: None,
        }))
    }};
    ($resource:expr, $outputs:expr) => {{
        Ok(Some(GetResourceOutput {
            resource_definition: Resource::to_string(&$resource)?,
            outputs: Some(HashMap::from_iter($outputs.into_iter().map(|(k, v)| (k.to_string(), v)))),
        }))
    }};
}

#[macro_export]
macro_rules! connector_op {
    ($op:expr, $message:expr) => {{
        OpPlanOutput {
            op_definition: ConnectorOp::to_string(&$op)?,
            writes_outputs: Vec::new(),
            friendly_message: Some($message),
        }
    }};
    ($op:expr, $outputs:expr, $message:expr) => {{
        OpPlanOutput {
            op_definition: ConnectorOp::to_string(&$op)?,
            writes_outputs: $outputs,
            friendly_message: Some($message),
        }
    }};
}

#[macro_export]
macro_rules! op_exec_output {
    ($outputs:expr, $message:expr) => {{
        Ok(OpExecOutput {
            outputs: $outputs.map::<HashMap<String, Option<String>>, _>(|o| {
                HashMap::from_iter(
                    o.into_iter()
                        .map::<(String, Option<String>), _>(|(k, v)| (k.to_string(), v.map::<String, _>(|v| v.to_string()))),
                )
            }),
            friendly_message: Some($message),
        })
    }};
    ($message:expr) => {{
        Ok(OpExecOutput {
            outputs: None,
            friendly_message: Some($message),
        })
    }};
}

#[macro_export]
macro_rules! skeleton {
    ($addr:expr, $resource:expr) => {{
        SkeletonOutput {
            addr: $addr.to_path_buf(),
            body: $resource.to_string()?,
        }
    }};
}
