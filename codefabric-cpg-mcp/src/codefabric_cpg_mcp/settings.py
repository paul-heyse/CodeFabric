"""Immutable process-lifetime settings for the STDIO adapter."""

from __future__ import annotations

import secrets
from typing import Annotated, Literal, cast

from pydantic import AliasChoices, Field, SecretStr, model_validator
from pydantic_settings import (
    BaseSettings,
    PydanticBaseSettingsSource,
    SettingsConfigDict,
)

OpaqueId = Annotated[
    str,
    Field(
        min_length=1,
        max_length=256,
        pattern=r"^[A-Za-z0-9][A-Za-z0-9_.:-]*$",
    ),
]


class Settings(BaseSettings):
    """Validated, frozen adapter settings loaded once during server lifespan."""

    model_config = SettingsConfigDict(
        case_sensitive=True,
        extra="ignore",
        frozen=True,
        validate_default=True,
        hide_input_in_errors=True,
        validate_by_alias=True,
        validate_by_name=True,
        env_file=None,
        cli_parse_args=False,
    )

    daemon_target: Annotated[str, Field(min_length=1, max_length=2048)] = Field(
        validation_alias=AliasChoices(
            "CODEFABRIC_CPG_DAEMON_TARGET",
            "CODEFABRIC_DAEMON_TARGET",
        )
    )
    workspace_id: OpaqueId = Field(validation_alias="CODEFABRIC_WORKSPACE_ID")
    agent_instance_id: OpaqueId = Field(
        default_factory=lambda: f"stdio-{secrets.token_hex(8)}",
        validation_alias="CODEFABRIC_AGENT_INSTANCE_ID",
    )
    capability_token: SecretStr = Field(validation_alias="CODEFABRIC_CPG_CAPABILITY_TOKEN")

    query_timeout_seconds: Annotated[float, Field(gt=0, le=600)] = Field(
        default=120.0,
        validation_alias="CODEFABRIC_CPG_QUERY_TIMEOUT_SECONDS",
    )
    inline_result_bytes: Annotated[int, Field(ge=16 * 1024, le=8 * 1024 * 1024)] = Field(
        default=384 * 1024,
        validation_alias="CODEFABRIC_CPG_INLINE_RESULT_BYTES",
    )
    max_request_bytes: Annotated[int, Field(ge=64 * 1024, le=16 * 1024 * 1024)] = Field(
        default=2 * 1024 * 1024,
        validation_alias="CODEFABRIC_CPG_MAX_REQUEST_BYTES",
    )
    max_json_depth: Annotated[int, Field(ge=4, le=128)] = Field(
        default=48,
        validation_alias="CODEFABRIC_CPG_MAX_JSON_DEPTH",
    )
    max_json_nodes: Annotated[int, Field(ge=100, le=2_000_000)] = Field(
        default=100_000,
        validation_alias="CODEFABRIC_CPG_MAX_JSON_NODES",
    )
    max_validation_errors: Annotated[int, Field(ge=1, le=100)] = Field(
        default=20,
        validation_alias="CODEFABRIC_CPG_MAX_VALIDATION_ERRORS",
    )
    result_ttl_seconds: Annotated[int, Field(ge=60, le=86_400)] = Field(
        default=1_800,
        validation_alias="CODEFABRIC_CPG_RESULT_TTL_SECONDS",
    )
    log_level: Literal["CRITICAL", "ERROR", "WARNING", "INFO", "DEBUG"] = Field(
        default=cast(
            Literal["CRITICAL", "ERROR", "WARNING", "INFO", "DEBUG"],
            "INFO",
        ),
        validation_alias="CODEFABRIC_CPG_LOG_LEVEL",
    )

    @model_validator(mode="after")
    def validate_daemon_target(self) -> Settings:
        """Restrict the daemon endpoint to the two protocol-owned schemes."""

        if not self.daemon_target.startswith(("unix://", "tcp://")):
            raise ValueError("daemon_target must use unix:// or tcp://")
        return self

    @classmethod
    def settings_customise_sources(
        cls,
        settings_cls: type[BaseSettings],
        init_settings: PydanticBaseSettingsSource,
        env_settings: PydanticBaseSettingsSource,
        dotenv_settings: PydanticBaseSettingsSource,
        file_secret_settings: PydanticBaseSettingsSource,
    ) -> tuple[PydanticBaseSettingsSource, ...]:
        """Lock source priority and deliberately exclude dotenv loading."""

        return init_settings, env_settings, file_secret_settings
