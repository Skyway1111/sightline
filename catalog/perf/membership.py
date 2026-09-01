"""#41 catalog entry `list-attr-membership`: the slow exemplar IS the matched
shape, its fast twin is not. The pair's shape lives in its two classes."""


class MembershipSlow:  # sightline-ok: 11
    def __init__(self):
        self.seen = []

    def add_all(self, items):
        for x in items:
            if x in self.seen:
                continue
            self.seen.append(x)
        return sorted(self.seen)


class MembershipFast:  # sightline-ok: 11
    def __init__(self):
        self.seen = set()

    def add_all(self, items):
        for x in items:
            if x in self.seen:
                continue
            self.seen.add(x)
        return sorted(self.seen)


def membership_slow(items):
    return MembershipSlow().add_all(items)


def membership_fast(items):
    return MembershipFast().add_all(items)
