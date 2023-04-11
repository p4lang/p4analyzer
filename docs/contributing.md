# Contributing to P4 Analyzer

## Logging Issues
Before you log a new issue, [search the existing issues](https://github.com/p4lang/p4analyzer/search?type=Issues) first. Use the following search tips to help determine if your issue has been encountered before:

* *Don't* restrict your search to only open issues. An issue with a title similar to yours may have been closed as a duplicate of one with a less-findable title.
* Search for the title of the issue you're about to log. This sounds obvious but 80% of the time this is sufficient to find a duplicate when one exists.
* Read more than the first page of results. Many bugs here use the same words so relevancy sorting is not particularly strong.
* If you have a crash, search for the first few top-most function names shown in any call stack.

### Found a bug?
When logging a bug, please include the version number of the Visual Studio Code extension that you are using. If you are running P4 Analyzer stand-alone (i.e., you are configuring it as part of a VIM, Emacs, or other LSP client setup), then run '`p4analyzer --version`' to retrieve the version number.

If possible, try and include an _isolated_ way in which the bug can be reproduced, and describe both the expected and actual behaviors.

## Have a 'pitch' for a feature or idea?

We also accept _pitches_ for features or ideas as issues. When creating a _pitch_, please ensure to apply the `'pitch'` label to it so that we can triage and track it on this [Pitches GitHub Project](https://github.com/orgs/p4lang/projects/1/views/1).

We use the [Pitches GitHub Project](https://github.com/orgs/p4lang/projects/1/views/1) to roadmap planned development effort, and as pitches move from _draft_ into _shaping_, we anticipate discussion and questions appearing on your particular _pitch_ issue to help with that shaping.

More information about how we process, manage and ultimately deliver pitches can be found in the [Issue Life-Cycle](./issue-lifecycle.md).
