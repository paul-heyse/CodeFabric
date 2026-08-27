from golden_pkg.core import pipeline  # @anchor ffi.import.pipeline


def exported_pipeline(value: int) -> int:  # @anchor ffi.exported_pipeline
    return pipeline(value)  # @anchor ffi.call.pipeline
