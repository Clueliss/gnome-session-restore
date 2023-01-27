#!/bin/bash

find $(echo $XDG_DATA_DIRS:$(echo ~/.local/share): | sed 's|:|/applications |g') -maxdepth 1 -xtype f || true
