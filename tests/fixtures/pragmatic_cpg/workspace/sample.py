from helpers import twice


def increment(value: int) -> int:
    return value + 1


def pipeline(value: int) -> int:
    return twice(increment(value))


def dynamic_call(callback, value):
    return callback(value)
