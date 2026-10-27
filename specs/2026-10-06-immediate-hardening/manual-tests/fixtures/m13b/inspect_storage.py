"""Read-only S3/GuardDuty configuration inspection; no object or policy writes."""

import argparse
import json
import os
from datetime import datetime, timezone
from pathlib import Path

import boto3
from botocore.config import Config
from botocore.exceptions import BotoCoreError, ClientError, NoCredentialsError


def check(operation, **parameters):
    try:
        result = operation(**parameters)
    except NoCredentialsError:
        raise
    except ClientError as error:
        return {
            "inspection": "unverified",
            "error_code": error.response.get("Error", {}).get("Code"),
            "http_status": error.response.get("ResponseMetadata", {}).get("HTTPStatusCode"),
        }
    except BotoCoreError as error:
        return {"inspection": "unverified", "error_code": type(error).__name__}
    result.pop("ResponseMetadata", None)
    if isinstance(result.get("Policy"), str):
        result["Policy"] = json.loads(result["Policy"])
    return result


def guardduty_plans(client, bucket):
    plans = []
    parameters = {}
    while True:
        page = check(client.list_malware_protection_plans, **parameters)
        if "inspection" in page:
            return {"plans": plans, "listing": page}
        for item in page.get("MalwareProtectionPlans", []):
            identity = item["MalwareProtectionPlanId"]
            detail = check(client.get_malware_protection_plan, MalwareProtectionPlanId=identity)
            if "inspection" in detail:
                plans.append({"plan_id": identity, "details": detail})
            elif (
                detail.get("ProtectedResource", {}).get("S3Bucket", {}).get("BucketName") == bucket
            ):
                plans.append({"plan_id": identity, "details": detail})
        token = page.get("NextToken")
        if not token:
            return {"plans": plans, "listing": "complete_in_selected_region"}
        parameters["NextToken"] = token


def inspect(args, session):
    config = Config(connect_timeout=10, read_timeout=20, retries={"max_attempts": 1})
    identity = check(
        session.client("sts", region_name=args.region, config=config).get_caller_identity
    )
    if "inspection" in identity:
        raise ValueError("AWS identity could not be verified: " + json.dumps(identity))
    storage = session.client("s3", region_name=args.region, config=config)
    location = check(storage.get_bucket_location, Bucket=args.bucket)
    region = location.get("LocationConstraint") or args.region
    if region == "EU":
        region = "eu-west-1"
    if "inspection" not in location and location.get("LocationConstraint") is None:
        region = "us-east-1"
    report = {
        "checked_at": datetime.now(timezone.utc).isoformat(),
        "bucket": args.bucket,
        "region": region,
        "region_basis": "bucket_location" if "inspection" not in location else "argument_fallback",
        "caller": identity,
        "scope": "Configuration reads only; no PUT, DELETE, tag, policy or database mutations.",
        "registry_permissions": (
            "Unverified: the inspection caller is not assumed to be Render's Registry principal. "
            "Bucket policy alone does not establish effective identity/KMS permissions."
        ),
        "live_delivery": "Unverified: no new objects are created or scanned by this inspection.",
        "location": location,
    }
    operations = {
        "lifecycle": storage.get_bucket_lifecycle_configuration,
        "bucket_policy": storage.get_bucket_policy,
        "versioning": storage.get_bucket_versioning,
        "encryption": storage.get_bucket_encryption,
        "public_access_block": storage.get_public_access_block,
        "notifications": storage.get_bucket_notification_configuration,
    }
    for label, operation in operations.items():
        report[label] = check(operation, Bucket=args.bucket)
    guardduty = session.client("guardduty", region_name=region, config=config)
    report["guardduty"] = guardduty_plans(guardduty, args.bucket)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--bucket", choices=("agentpm-tools", "agentpm-dev-tools"), required=True)
    parser.add_argument("--region", default="us-east-1")
    parser.add_argument("--profile")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    session = boto3.Session(profile_name=args.profile, region_name=args.region)
    report = inspect(args, session)
    # A local report is the only write; refuse to replace earlier evidence.
    with open(args.output, "x", opener=lambda path, flags: os.open(path, flags, 0o600)) as output:
        json.dump(report, output, indent=2, default=str)
        output.write("\n")
    print(json.dumps(report, indent=2, default=str))
    print("Saved read-only inspection:", args.output)


if __name__ == "__main__":
    try:
        main()
    except NoCredentialsError:
        raise SystemExit(
            "No AWS credentials in this terminal. Use the terminal where credentials were exported "
            "or select an existing AWS profile. Do not paste credentials into chat."
        ) from None
    except (BotoCoreError, ValueError, OSError) as error:
        raise SystemExit(str(error)) from None
