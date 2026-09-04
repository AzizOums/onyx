from tests.integration.common_utils.constants import API_SERVER_URL
from tests.integration.common_utils.http_client import client
from tests.integration.common_utils.test_models import DATestUser


class DocumentSearchManager:
    @staticmethod
    def search_documents(
        query: str,
        user_performing_action: DATestUser,
    ) -> list[str]:
        """
        Search for documents using the Community Edition search API.

        Args:
            query: The search query string
            user_performing_action: The user performing the search (for auth)

        Returns:
            A list of document content strings from the search results
        """
        result = client.post(
            url=f"{API_SERVER_URL}/search",
            json={"query": query},
            headers=user_performing_action.headers,
        )
        result.raise_for_status()
        result_json = result.json()

        # Return the content of each result section
        document_content_list: list[str] = [
            section["content"] for section in result_json["results"]
        ]
        return document_content_list
