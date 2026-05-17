use crate::model::{
    AchitekFile, Blueprint, Prompt, Spanned, ValidAchitekFile, ValidBlueprint, ValidPrompt,
};

pub(super) fn validate_file(file: AchitekFile) -> ValidAchitekFile {
    let blueprint = validate_blueprint(file.blueprint());
    let prompts = file
        .prompts()
        .iter()
        .map(validate_prompt)
        .collect::<Vec<_>>();

    ValidAchitekFile::new(blueprint, prompts)
}

fn validate_blueprint(blueprint: &Blueprint) -> ValidBlueprint {
    ValidBlueprint {
        version: blueprint
            .version
            .as_ref()
            .expect("analysis should reject blueprints without a version")
            .value
            .clone(),
        name: blueprint
            .name
            .as_ref()
            .expect("analysis should reject blueprints without a name")
            .value
            .clone(),
        description: blueprint
            .description
            .as_ref()
            .map(|description| description.value.clone()),
        author: blueprint.author.as_ref().map(|author| author.value.clone()),
        min_achitek_version: blueprint
            .min_achitek_version
            .as_ref()
            .map(|version| version.value.clone()),
    }
}

fn validate_prompt(prompt: &Spanned<Prompt>) -> ValidPrompt {
    let prompt = &prompt.value;

    ValidPrompt {
        name: prompt.name.clone(),
        prompt_type: prompt
            .prompt_type
            .expect("analysis should reject prompts without a type"),
        help: prompt.help.clone(),
        choices: prompt.choices.clone(),
        default: prompt.default.clone(),
        required: prompt.required.unwrap_or(false),
        depends_on: prompt.depends_on.clone(),
        validation: prompt.validation.clone(),
    }
}
