# Runtime manifests

Runtime acquisition policy is separate from runtime execution. These manifests
describe the current implementation and redistribution posture; they do not
install runtimes or game material.

`releaseAllowed: false` is an explicit gate. It must not be changed until the
adapter exists, compatibility is tested, and license/distribution review is
recorded. External and manual runtimes must work from user-selected
installations without FTEP downloading proprietary material.
