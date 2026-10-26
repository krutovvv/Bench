#!/bin/sh
# An ssh stand-in for tests: its options and the host are dropped, the command runs here.
while [ $# -gt 0 ]; do
	case "$1" in
	-o) shift 2 ;;
	-*) shift ;;
	*) shift; break ;;
	esac
done
exec sh -c "$*"
