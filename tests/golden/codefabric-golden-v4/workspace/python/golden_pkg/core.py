def scale(value: int) -> int:  # @anchor py.scale
    return value * 2


def pipeline(value: int) -> int:  # @anchor py.pipeline
    return scale(value) + 1  # @anchor py.call.scale


class Counter:  # @anchor py.counter
    def increment(self, value: int) -> int:  # @anchor py.counter.increment
        return value + 1
