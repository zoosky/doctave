use crate::config::{Config, DirIncludeRule, NavRule};
use crate::Directory;
use serde::Serialize;

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

pub struct Navigation<'a> {
    config: &'a Config,
}

impl<'a> Navigation<'a> {
    pub fn new(config: &'a Config) -> Self {
        Navigation { config }
    }

    pub fn build_for(&self, dir: &Directory) -> Vec<Link> {
        let default: Vec<Link> = dir.into();

        match &self.config.navigation() {
            None => default,
            Some(nav) => self.customize(nav, &default),
        }
    }

    fn customize(&self, rules: &[NavRule], default: &[Link]) -> Vec<Link> {
        let mut links = vec![];

        for rule in rules {
            match rule {
                NavRule::File(path) => links.push(self.find_matching_link(path, &default)),
                NavRule::Dir(path, dir_rule) => {
                    let mut index_link = self.find_matching_link(path, &default);

                    match dir_rule {
                        // Don't include any children
                        None => {
                            index_link.children.truncate(0);
                            links.push(index_link);
                        }
                        // Include all children
                        Some(DirIncludeRule::WildCard) => links.push(index_link),
                        // Include only children that match the description
                        Some(DirIncludeRule::Explicit(nested_rules)) => {
                            let children = self.customize(nested_rules, &index_link.children);
                            index_link.children = children;
                            links.push(index_link);
                        }
                    }
                }
            }
        }

        links
    }

    fn find_matching_link(&self, path: &Path, links: &[Link]) -> Link {
        links
            .iter()
            .find(|link| {
                let mut without_docs_part = path.components();
                let _ = without_docs_part.next();

                link.path == Link::path_to_uri(without_docs_part.as_path())
            })
            .expect("Could not find matching doc for rule")
            .clone()
    }
}

impl From<&Directory> for Vec<Link> {
    fn from(dir: &Directory) -> Vec<Link> {
        let mut links = dir
            .docs
            .iter()
            .map(|d| Link {
                title: d.title().to_owned(),
                path: Link::path_to_uri(&d.html_path()),
                children: vec![],
            })
            .filter(|l| l.path != Link::path_to_uri(&dir.index().html_path()))
            .collect::<Vec<_>>();

        let mut children = dir
            .dirs
            .iter()
            .map(|d| Link {
                title: d.index().title().to_owned(),
                path: Link::path_to_uri(&d.index().html_path()),
                children: d.into(),
            })
            .collect::<Vec<_>>();

        links.append(&mut children);
        links.sort_by(|a, b| alphanumeric_sort::compare_str(&a.title, &b.title));

        links
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Link {
    pub path: String,
    pub title: String,
    pub children: Vec<Link>,
}

impl Link {
    pub fn path_to_uri(path: &Path) -> String {
        let mut tmp = path.to_owned();

        // Default to stipping .html extensions
        tmp.set_extension("");

        if tmp.file_name() == Some(OsStr::new("index")) {
            tmp = tmp
                .parent()
                .map(|p| p.to_owned())
                .unwrap_or_else(|| PathBuf::from(""));
        }

        // Need to force forward slashes here, since URIs will always
        // work the same across all platforms.
        let uri_path = tmp
            .components()
            .into_iter()
            .map(|c| format!("{}", c.as_os_str().to_string_lossy()))
            .collect::<Vec<_>>()
            .join("/");

        format!("/{}", uri_path)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::collections::BTreeMap;
    use std::path::Path;

    use crate::Document;

    fn page(path: &str, name: &str) -> Document {
        let mut frontmatter = BTreeMap::new();
        frontmatter.insert("title".to_string(), name.to_string());

        Document::new(Path::new(path), "Not important".to_string(), frontmatter)
    }

    fn config(yaml: Option<&str>) -> Config {
        let conf = yaml.unwrap_or("---\ntitle: My project\n");

        Config::from_yaml_str(&Path::new("project"), conf).unwrap()
    }

    #[test]
    fn basic() {
        let config = config(None);
        let root = Directory {
            path: PathBuf::from("docs"),
            docs: vec![
                page("README.md", "Getting Started"),
                page("one.md", "One"),
                page("two.md", "Two"),
            ],
            dirs: vec![Directory {
                path: PathBuf::from("docs").join("child"),
                docs: vec![
                    page("child/README.md", "Nested Root"),
                    page("child/three.md", "Three"),
                ],
                dirs: vec![],
            }],
        };

        let navigation = Navigation::new(&config);

        assert_eq!(
            navigation.build_for(&root),
            vec![
                Link {
                    path: String::from("/child"),
                    title: String::from("Nested Root"),
                    children: vec![Link {
                        path: String::from("/child/three"),
                        title: String::from("Three"),
                        children: vec![]
                    }]
                },
                Link {
                    path: String::from("/one"),
                    title: String::from("One"),
                    children: vec![]
                },
                Link {
                    path: String::from("/two"),
                    title: String::from("Two"),
                    children: vec![]
                },
            ]
        )
    }

    #[test]
    fn sorting_alphanumerically() {
        let config = config(None);
        let root = Directory {
            path: PathBuf::from("docs"),
            docs: vec![
                page("README.md", "Getting Started"),
                page("001.md", "bb"),
                page("002.md", "11"),
            ],
            dirs: vec![
                Directory {
                    path: PathBuf::from("docs").join("bb_child"),
                    docs: vec![
                        page("child/README.md", "Index"),
                        page("child/001.md", "BB"),
                        page("child/002.md", "22"),
                        page("child/003.md", "AA"),
                        page("child/004.md", "11"),
                    ],
                    dirs: vec![],
                },
                Directory {
                    path: PathBuf::from("docs").join("aa_child"),
                    docs: vec![
                        page("child2/README.md", "Index"),
                        page("child2/001.md", "123"),
                        page("child2/002.md", "aa"),
                        page("child2/003.md", "cc"),
                        page("child2/004.md", "bb"),
                    ],
                    dirs: vec![],
                },
            ],
        };

        let navigation = Navigation::new(&config);

        assert_eq!(
            navigation.build_for(&root),
            vec![
                Link {
                    path: String::from("/002"),
                    title: String::from("11"),
                    children: vec![],
                },
                Link {
                    path: String::from("/child"),
                    title: String::from("Index"),
                    children: vec![
                        Link {
                            path: String::from("/child/004"),
                            title: String::from("11"),
                            children: vec![],
                        },
                        Link {
                            path: String::from("/child/002"),
                            title: String::from("22"),
                            children: vec![],
                        },
                        Link {
                            path: String::from("/child/003"),
                            title: String::from("AA"),
                            children: vec![],
                        },
                        Link {
                            path: String::from("/child/001"),
                            title: String::from("BB"),
                            children: vec![],
                        },
                    ]
                },
                Link {
                    path: String::from("/child2"),
                    title: String::from("Index"),
                    children: vec![
                        Link {
                            path: String::from("/child2/001"),
                            title: String::from("123"),
                            children: vec![]
                        },
                        Link {
                            path: String::from("/child2/002"),
                            title: String::from("aa"),
                            children: vec![]
                        },
                        Link {
                            path: String::from("/child2/004"),
                            title: String::from("bb"),
                            children: vec![]
                        },
                        Link {
                            path: String::from("/child2/003"),
                            title: String::from("cc"),
                            children: vec![]
                        },
                    ]
                },
                Link {
                    path: String::from("/001"),
                    title: String::from("bb"),
                    children: vec![],
                },
            ],
        )
    }

    #[test]
    fn manual_menu_simple() {
        let root = Directory {
            path: PathBuf::from("docs"),
            docs: vec![
                page("README.md", "Getting Started"),
                page("one.md", "One"),
                page("two.md", "Two"),
            ],
            dirs: vec![Directory {
                path: PathBuf::from("docs").join("child"),
                docs: vec![
                    page("child/README.md", "Nested Root"),
                    page("child/three.md", "Three"),
                ],
                dirs: vec![],
            }],
        };

        let rules = vec![
            NavRule::File(PathBuf::from("docs/one.md")),
            NavRule::Dir(PathBuf::from("docs/child"), Some(DirIncludeRule::WildCard)),
        ];

        let config = config(None);
        let navigation = Navigation::new(&config);
        let links: Vec<Link> = (&root).into();

        assert_eq!(
            navigation.customize(&rules, &links),
            vec![
                Link {
                    path: String::from("/one"),
                    title: String::from("One"),
                    children: vec![],
                },
                Link {
                    path: String::from("/child"),
                    title: String::from("Nested Root"),
                    children: vec![Link {
                        path: String::from("/child/three"),
                        title: String::from("Three"),
                        children: vec![],
                    },],
                },
            ]
        )
    }

    #[test]
    fn manual_menu_nested() {
        let root = Directory {
            path: PathBuf::from("docs"),
            docs: vec![
                page("README.md", "Getting Started"),
                page("one.md", "One"),
                page("two.md", "Two"),
            ],
            dirs: vec![Directory {
                path: PathBuf::from("docs").join("child"),
                docs: vec![
                    page("child/README.md", "Nested Root"),
                    page("child/three.md", "Three"),
                ],
                dirs: vec![Directory {
                    path: PathBuf::from("docs").join("child").join("nested"),
                    docs: vec![
                        page("child/nested/README.md", "Nested Root"),
                        page("child/nested/four.md", "Four"),
                    ],
                    dirs: vec![],
                }],
            }],
        };

        let rules = vec![
            NavRule::File(PathBuf::from("docs").join("one.md")),
            NavRule::Dir(
                PathBuf::from("docs").join("child"),
                Some(DirIncludeRule::Explicit(vec![NavRule::Dir(
                    PathBuf::from("docs").join("child").join("nested"),
                    Some(DirIncludeRule::Explicit(vec![NavRule::File(
                        PathBuf::from("docs")
                            .join("child")
                            .join("nested")
                            .join("four.md"),
                    )])),
                )])),
            ),
        ];

        let config = config(None);
        let navigation = Navigation::new(&config);
        let links: Vec<Link> = (&root).into();

        assert_eq!(
            navigation.customize(&rules, &links),
            vec![
                Link {
                    path: String::from("/one"),
                    title: String::from("One"),
                    children: vec![]
                },
                Link {
                    path: String::from("/child"),
                    title: String::from("Nested Root"),
                    children: vec![Link {
                        path: String::from("/child/nested"),
                        title: String::from("Nested Root"),
                        children: vec![Link {
                            path: String::from("/child/nested/four"),
                            title: String::from("Four"),
                            children: vec![]
                        },]
                    }]
                }
            ]
        );
    }
}
