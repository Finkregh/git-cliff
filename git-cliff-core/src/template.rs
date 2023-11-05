use crate::commit::Commit;
use crate::error::{
	Error,
	Result,
};
use crate::release::Release;
use indexmap::IndexMap;
use itertools::Itertools as _;
use std::collections::HashMap;
use std::error::Error as ErrorImpl;
use tera::{
	Context as TeraContext,
	Result as TeraResult,
	Tera,
	Value,
};

/// Wrapper for [`Tera`].
#[derive(Debug)]
pub struct Template {
	tera: Tera,
}

impl Template {
	/// Constructs a new instance.
	pub fn new(template: String) -> Result<Self> {
		let mut tera = Tera::default();
		if let Err(e) = tera.add_raw_template("template", &template) {
			return if let Some(error_source) = e.source() {
				Err(Error::TemplateParseError(error_source.to_string()))
			} else {
				Err(Error::TemplateError(e))
			};
		}
		tera.register_filter("upper_first", Self::upper_first_filter);
		tera.register_filter("commit_groups", Self::commit_groups);
		Ok(Self { tera })
	}

	/// Filter for making the first character of a string uppercase.
	fn commit_groups(
		value: &Value,
		args: &HashMap<String, Value>,
	) -> TeraResult<Value> {
		let mut commits =
			tera::try_get_value!("commit_groups", "value", Vec<Commit>, value);
		let groups_ordered_filter = tera::try_get_value!(
			"commit_groups",
			"groups",
			Vec<String>,
			args.get("groups").expect("groups must be present in args")
		);

		commits.retain(|commit| {
			groups_ordered_filter.iter().any(|group| {
				group == commit.group.as_deref().expect("commit must have group")
			})
		});

		let groups = commits
			.into_iter()
			.into_group_map_by(|commit| {
				commit.group.clone().expect("commit must have group")
			})
			.into_iter()
			.sorted_by_key(|(group_name, _)| {
				groups_ordered_filter
					.iter()
					.position(|group| group_name == group)
					.expect("commits have already been filtered to specified groups")
			});

		let mut ordered_groups = IndexMap::new();

		for (group, commits) in groups {
			dbg!((&group, &commits));
			ordered_groups.insert(group, commits);
		}

		Ok(tera::to_value(ordered_groups)?)
	}

	/// Filter for making the first character of a string uppercase.
	fn upper_first_filter(
		value: &Value,
		_: &HashMap<String, Value>,
	) -> TeraResult<Value> {
		let mut s =
			tera::try_get_value!("upper_first_filter", "value", String, value);
		let mut c = s.chars();
		s = match c.next() {
			None => String::new(),
			Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
		};
		Ok(tera::to_value(&s)?)
	}

	/// Renders the template.
	pub fn render(&self, release: &Release) -> Result<String> {
		let context = TeraContext::from_serialize(release)?;
		match self.tera.render("template", &context) {
			Ok(v) => Ok(v),
			Err(e) => {
				return if let Some(error_source) = e.source() {
					Err(Error::TemplateRenderError(error_source.to_string()))
				} else {
					Err(Error::TemplateError(e))
				};
			}
		}
	}

	/// Renders the template.
	pub fn render_with_groups(
		&self,
		release: &Release,
		groups: &[&str],
	) -> Result<String> {
		let mut context = TeraContext::from_serialize(release)?;
		context.insert("commit_groups_filter", groups);
		match self.tera.render("template", &context) {
			Ok(v) => Ok(v),
			Err(e) => {
				return if let Some(error_source) = e.source() {
					Err(Error::TemplateRenderError(error_source.to_string()))
				} else {
					Err(Error::TemplateError(e))
				};
			}
		}
	}
}

#[cfg(test)]
mod test {
	use super::*;
	use crate::commit::Commit;

	#[test]
	fn render_template() -> Result<()> {
		let template = r#"
		## {{ version }}
		{% for commit in commits %}
		### {{ commit.group }}
		- {{ commit.message | upper_first }}
		{% endfor %}"#;
		let template = Template::new(template.to_string())?;
		assert_eq!(
			r#"
		## 1.0
		
		### feat
		- Add xyz
		
		### fix
		- Fix abc
		"#,
			template.render(&Release {
				version:   Some(String::from("1.0")),
				commits:   vec![
					Commit::new(
						String::from("123123"),
						String::from("feat(xyz): add xyz"),
					),
					Commit::new(
						String::from("124124"),
						String::from("fix(abc): fix abc"),
					)
				]
				.into_iter()
				.filter_map(|c| c.into_conventional().ok())
				.collect(),
				commit_id: None,
				timestamp: 0,
				previous:  None,
			})?
		);
		Ok(())
	}
}
