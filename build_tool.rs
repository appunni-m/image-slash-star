//! Pure environment-resolution helpers shared by the AVIF build script tests.

pub(crate) fn target_tool_env_names(variable: &str, target: &str) -> [String; 3] {
    [
        format!("{variable}_{}", target.replace('-', "_")),
        format!("TARGET_{variable}"),
        variable.to_owned(),
    ]
}

pub(crate) fn target_tool_from_lookup<F>(
    variable: &str,
    target: &str,
    fallback: &str,
    mut lookup: F,
) -> String
where
    F: FnMut(&str) -> Option<String>,
{
    target_tool_env_names(variable, target)
        .into_iter()
        .find_map(|name| lookup(&name))
        .unwrap_or_else(|| fallback.to_owned())
}
