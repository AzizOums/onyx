"""Factory stub for running the Craft scheduled-tasks Celery worker."""

from celery import Celery

from lumen.utils.variable_functionality import (
    fetch_versioned_implementation,
    set_is_ee_based_on_env_variable,
)

set_is_ee_based_on_env_variable()
app: Celery = fetch_versioned_implementation(
    "lumen.background.celery.apps.scheduled_tasks",
    "celery_app",
)
