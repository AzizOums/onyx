# Named `platform_utils` (not `platform`) so direct execution of a utility from this
# directory cannot shadow the stdlib `platform`. See issue #10975.
import os
import warnings

_LUMEN_DOCKER_ENV_STR = "LUMEN_RUNNING_IN_DOCKER"
_DANSWER_DOCKER_ENV_STR = "DANSWER_RUNNING_IN_DOCKER"


def _resolve_container_flag() -> bool:
    lumen_val = os.getenv(_LUMEN_DOCKER_ENV_STR)
    if lumen_val is not None:
        return lumen_val.lower() == "true"

    danswer_val = os.getenv(_DANSWER_DOCKER_ENV_STR)
    if danswer_val is not None:
        warnings.warn(
            f"{_DANSWER_DOCKER_ENV_STR} is deprecated and will be ignored in a "
            f"future release. Use {_LUMEN_DOCKER_ENV_STR} instead.",
            DeprecationWarning,
            stacklevel=2,
        )
        return danswer_val.lower() == "true"

    return False


_IS_RUNNING_IN_CONTAINER: bool = _resolve_container_flag()
_IS_RUNNING_IN_KUBERNETES: bool = os.getenv("KUBERNETES_SERVICE_HOST") is not None


def is_running_in_container() -> bool:
    return _IS_RUNNING_IN_CONTAINER


def is_running_in_kubernetes() -> bool:
    return _IS_RUNNING_IN_KUBERNETES
