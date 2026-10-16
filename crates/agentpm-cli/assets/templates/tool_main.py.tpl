import json
import sys


def main():
    request = json.load(sys.stdin)
    print(json.dumps(request))


if __name__ == "__main__":
    main()
