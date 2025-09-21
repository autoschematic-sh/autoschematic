#[macro_export]
macro_rules! get_resource_response {
    ($resource:expr) => {{
        Ok(Some(GetResourceResponse {
            resource_definition: Resource::to_bytes(&$resource).context("Resource::to_bytes")?,
            outputs: None,
        }))
    }};
    ($resource:expr, $outputs:expr) => {{
        Ok(Some(GetResourceResponse {
            resource_definition: Resource::to_bytes(&$resource).context("Resource::to_bytes")?,
            outputs: Some(HashMap::from_iter($outputs.into_iter().map(|(k, v)| (k.to_string(), v)))),
        }))
    }};
}

#[macro_export]
macro_rules! connector_op {
    ($op:expr, $message:expr) => {{
        PlanResponseElement {
            op_definition: ConnectorOp::to_string(&$op)?,
            writes_outputs: Vec::new(),
            friendly_message: Some($message),
        }
    }};
    ($op:expr, $outputs:expr, $message:expr) => {{
        PlanResponseElement {
            op_definition: ConnectorOp::to_string(&$op)?,
            writes_outputs: $outputs,
            friendly_message: Some($message),
        }
    }};
}

#[macro_export]
macro_rules! op_exec_output {
    ($outputs:expr, $message:expr) => {{
        Ok(OpExecResponse {
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
        Ok(OpExecResponse {
            outputs: None,
            friendly_message: Some($message),
        })
    }};
}

#[macro_export]
macro_rules! skeleton {
    ($addr:expr, $resource:expr) => {{
        SkeletonResponse {
            addr: $addr.to_path_buf(),
            body: $resource.to_bytes()?,
        }
    }};
}

#[macro_export]
macro_rules! virt_to_phy {
    (
        $enum:path, $addr:ident, $prefix:expr,
        trivial => [ $( $triv_variant:ident { $triv_field:ident } ),* $(,)? ],
        null => [ $( $null_variant:ident { $null_field:ident } ),* $(,)? ],
        todo => [ $( $todo_variant:ident { $($todo_field:ident),* } ),* $(,)? ]
    ) => {
        match &$addr {
            $(
                <$enum>::$triv_variant { .. } => {
                    if let Some($triv_field) = $addr.get_output($prefix, stringify!($triv_field))? {
                        Ok(VirtToPhyResponse::Present(
                            <$enum>::$triv_variant { $triv_field }.to_path_buf(),
                        ))
                    } else {
                        Ok(VirtToPhyResponse::NotPresent)
                    }
                }
            )*
            $(
                <$enum>::$null_variant { $null_field } => {
                    Ok(VirtToPhyResponse::Null(<$enum>::$null_variant { $null_field: $null_field.into() }.to_path_buf()))
                }
            )*
            $(
                <$enum>::$todo_variant { .. } => {
                    todo!()
                }
            )*
        }
    };
}
