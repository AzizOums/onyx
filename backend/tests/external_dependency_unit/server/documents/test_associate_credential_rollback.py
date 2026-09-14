"""The validation-failure arm of PUT /manage/connector/{id}/credential/{id} used to
delete any connector with no cc_pairs, which let a caller destroy someone else's
unassociated one. Not an integration test: INTEGRATION_TESTS_MODE makes
validate_ccpair_for_user return True before it can raise.
"""

from unittest.mock import patch
from uuid import uuid4

import pytest
from sqlalchemy import select
from sqlalchemy.exc import IntegrityError
from sqlalchemy.orm import Session

from lumen.configs.constants import DocumentSource
from lumen.connectors.exceptions import ConnectorValidationError
from lumen.connectors.models import InputType
from lumen.db.enums import AccessType
from lumen.db.models import Connector, Credential
from lumen.error_handling.exceptions import LumenError
from lumen.server.documents.cc_pair import associate_credential_to_connector
from lumen.server.documents.models import ConnectorCredentialPairMetadata
from shared_configs.configs import POSTGRES_DEFAULT_SCHEMA_STANDARD_VALUE
from tests.external_dependency_unit.conftest import create_test_user


def test_validation_failure_leaves_a_connector_this_flow_did_not_create(
    db_session: Session,
) -> None:
    caller = create_test_user(db_session, "associate_caller", is_admin=True)

    suffix = uuid4().hex[:8]
    # standalone, no cc_pair — the state POST /admin/connector leaves behind
    connector = Connector(
        name=f"orphan-connector-{suffix}",
        source=DocumentSource.MOCK_CONNECTOR,
        input_type=InputType.LOAD_STATE,
        connector_specific_config={},
        refresh_freq=None,
        prune_freq=None,
        indexing_start=None,
    )
    credential = Credential(
        source=DocumentSource.MOCK_CONNECTOR,
        credential_json={},
        user_id=caller.id,
    )
    db_session.add_all([connector, credential])
    db_session.commit()

    with patch(
        "lumen.server.documents.cc_pair.validate_ccpair_for_user",
        side_effect=ConnectorValidationError("bad settings"),
    ):
        with pytest.raises(LumenError):
            associate_credential_to_connector(
                connector_id=connector.id,
                credential_id=credential.id,
                metadata=ConnectorCredentialPairMetadata(
                    name=f"pair-{suffix}",
                    access_type=AccessType.PUBLIC,
                ),
                user=caller,
                db_session=db_session,
                tenant_id=POSTGRES_DEFAULT_SCHEMA_STANDARD_VALUE,
            )

    assert (
        db_session.scalar(select(Connector).where(Connector.id == connector.id))
        is not None
    )


def test_integrity_error_leaves_the_caller_s_connector_alone(
    db_session: Session,
) -> None:
    """The IntegrityError arm deleted the connector outright, citing a unique name
    constraint that 76b60d407dfb dropped in 2023."""
    caller = create_test_user(db_session, "integrity_caller", is_admin=True)

    suffix = uuid4().hex[:8]
    connector = Connector(
        name=f"orphan-connector-{suffix}",
        source=DocumentSource.MOCK_CONNECTOR,
        input_type=InputType.LOAD_STATE,
        connector_specific_config={},
        refresh_freq=None,
        prune_freq=None,
        indexing_start=None,
    )
    credential = Credential(
        source=DocumentSource.MOCK_CONNECTOR,
        credential_json={},
        user_id=caller.id,
    )
    db_session.add_all([connector, credential])
    db_session.commit()

    with patch(
        "lumen.server.documents.cc_pair.add_credential_to_connector",
        side_effect=IntegrityError("stmt", {}, Exception("duplicate key")),
    ):
        with pytest.raises(LumenError):
            associate_credential_to_connector(
                connector_id=connector.id,
                credential_id=credential.id,
                metadata=ConnectorCredentialPairMetadata(
                    name=f"pair-{suffix}",
                    access_type=AccessType.PUBLIC,
                ),
                user=caller,
                db_session=db_session,
                tenant_id=POSTGRES_DEFAULT_SCHEMA_STANDARD_VALUE,
            )

    assert (
        db_session.scalar(select(Connector).where(Connector.id == connector.id))
        is not None
    )
