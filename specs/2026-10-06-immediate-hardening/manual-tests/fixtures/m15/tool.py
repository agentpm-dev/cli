"""Use a real locked dependency; write only the Tool result to stdout."""

import json
import sys

from pydantic import BaseModel, ConfigDict, Field, ValidationError


class Order(BaseModel):
    model_config = ConfigDict(extra="forbid", allow_inf_nan=False)

    quantity: int = Field(gt=0, strict=True)
    unit_price: float = Field(gt=0, strict=True)


def main():
    try:
        order = Order.model_validate_json(sys.stdin.read())
    except ValidationError as error:
        print(error, file=sys.stderr)
        return 1
    print(json.dumps({"total": round(order.quantity * order.unit_price, 2)}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
