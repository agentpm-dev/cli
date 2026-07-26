{
  "$schema": "https://json-schema.org/draft/2020-12/schema",
  "type": "object",
  "properties": {
    "favorite_color": {
      "type": "string",
      "x-agentpm-data-class": "personal",
      "x-agentpm-sensitivity": "low",
      "x-agentpm-persist": true,
      "x-agentpm-shareable": false
    }
  },
  "required": ["favorite_color"],
  "additionalProperties": false
}
