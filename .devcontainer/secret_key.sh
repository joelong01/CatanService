#!/bin/bash
LOGIN_SECRET_KEY=$(openssl rand -hex 32)
export LOGIN_SECRET_KEY