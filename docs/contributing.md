# Contributing to P4 Analyzer

## Logging Issues
Before you log a new issue, [search the existing issues](https://github.com/p4lang/p4analyzer/search?type=Issues)
first. Use the following search tips to help determine if your issue has been encountered before:

* *Don't* restrict your search to only open issues. An issue with a title similar to yours may have been closed
as a duplicate of one with a less-findable title.
* Search for the title of the issue you're about to log. This sounds obvious but 80% of the time this is
sufficient to find a duplicate when one exists.
* Read more than the first page of results. Many bugs here use the same words so relevancy sorting is not
particularly strong.
* If you have a crash, search for the first few top-most function names shown in any call stack.

### Found a bug?
When logging a bug, please include the version number of the Visual Studio Code extension that you are using. If
you are running P4 Analyzer stand-alone (i.e., you are configuring it as part of a VIM, Emacs, or other LSP client
setup), then run '`p4analyzer --version`' to retrieve the version number.

If possible, try and include an _isolated_ way in which the bug can be reproduced, and describe both the expected
and actual behaviors. When creating your bug, please add the `'C-bug'` label to it.

## Have an idea for an improvement, or a new feature?
We also accept _proposals_ for features or ideas as issues. Use the 'Proposal' Issue template to ensure that your new
issue has the `'C-proposal'` label applied to it. Internally, we use private GitHub Projects to roadmap planned
development and as proposals are drafted, and then elaborated upon, we anticipate discussion and questions appearing
on your particular _proposal_ issue in order to help with that process.

More information about how we process, manage and ultimately deliver proposals can be found in the
[Issue Life-Cycle](./issue-lifecycle.md) under the _Proposal Issues_ section.

## Open to Pull Requests?
Yes! We'll generally review any submitted Pull Requests.

However, if you want to embark on implementing something large, then it may be best to consider opening a Proposal
Issue first. This will prevent you from working on something that the core P4 Analyzer team is already considering
or working on. It also helps to discuss your larger piece of work before you fully commit to it.

