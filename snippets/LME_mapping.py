from typing import Self
import json

class LME_Mapping:
    len: int
    indexes: dict[int, int]
    ids: dict[str, int]
    groups: dict[str, list[int]]

    def load_from_json(filepath: str) -> Self:
        with open(filepath) as f:
            json_data = json.load(f)
        LME_Mapping(json_data["len"], json_data["indexes"], json_data["ids"], json_data["groups"])

    def __init__(self, len, indexes, ids, groups):
        self.len = len
        self.indexes = indexes
        self.ids = ids
        self.groups = groups

    def convert_index(self, index: int) -> int | None:
        return self.indexes.get(index)

    def get_group_indexes(self, group_name: str) -> list[int] | None:
        return self.groups.get(group_name)
    
    def get_id_index(self, id_name: str) -> int | None:
        return self.ids.get(id_name)
    
    def convert_name_to_indexes(self, name: str) -> list[int] | None:
        id_table_result = self.get_id_index(name)
        return [id_table_result] if id_table_result is not None else self.get_group_indexes(name)    

    def convert_query_to_indexes(self, query: int | str) -> list[int] | None:
        if type(query) == int:
            result = self.convert_index(query)
            return [result] if result is not None else None
        else:
            return self.convert_name_to_indexes(query)
        
    def convert_queries(self, queries: list[int | str]) -> list[int]:
        query_results = [self.convert_query_to_indexes(query) for query in queries]
        if None in query_results:
            raise ValueError({
                "Message": "Follow names or indexes not found in mapping",
                "queries": [queries[bad_index] for bad_index, value in enumerate(query_results) if value is None]
            })
        else:
            return [item for result in query_results for item in result]