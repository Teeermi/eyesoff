eyesoff-ocr: ocr.swift
	swiftc -O ocr.swift -o eyesoff-ocr

install: eyesoff-ocr
	mkdir -p ~/.local/bin
	ln -sf "$(CURDIR)/eyesoff.py" ~/.local/bin/eyesoff

test: eyesoff-ocr
	python3 test.py

.PHONY: install test
