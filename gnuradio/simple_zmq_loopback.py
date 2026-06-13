#!/usr/bin/env python3
# -*- coding: utf-8 -*-

#
# SPDX-License-Identifier: GPL-3.0
#
# GNU Radio Python Flow Graph
# Title: Simple ZMQ Loopback
# Author: Jakob Kunzler
# GNU Radio version: 3.10.12.0

from PyQt5 import Qt
from gnuradio import qtgui
from PyQt5 import QtCore
from gnuradio import analog
from gnuradio import blocks
import math
from gnuradio import gr
from gnuradio.filter import firdes
from gnuradio.fft import window
import sys
import signal
from PyQt5 import Qt
from argparse import ArgumentParser
from gnuradio.eng_arg import eng_float, intx
from gnuradio import eng_notation
from gnuradio import zeromq
import sip
import threading



class simple_zmq_loopback(gr.top_block, Qt.QWidget):

    def __init__(self):
        gr.top_block.__init__(self, "Simple ZMQ Loopback", catch_exceptions=True)
        Qt.QWidget.__init__(self)
        self.setWindowTitle("Simple ZMQ Loopback")
        qtgui.util.check_set_qss()
        try:
            self.setWindowIcon(Qt.QIcon.fromTheme('gnuradio-grc'))
        except BaseException as exc:
            print(f"Qt GUI: Could not set Icon: {str(exc)}", file=sys.stderr)
        self.top_scroll_layout = Qt.QVBoxLayout()
        self.setLayout(self.top_scroll_layout)
        self.top_scroll = Qt.QScrollArea()
        self.top_scroll.setFrameStyle(Qt.QFrame.NoFrame)
        self.top_scroll_layout.addWidget(self.top_scroll)
        self.top_scroll.setWidgetResizable(True)
        self.top_widget = Qt.QWidget()
        self.top_scroll.setWidget(self.top_widget)
        self.top_layout = Qt.QVBoxLayout(self.top_widget)
        self.top_grid_layout = Qt.QGridLayout()
        self.top_layout.addLayout(self.top_grid_layout)

        self.settings = Qt.QSettings("gnuradio/flowgraphs", "simple_zmq_loopback")

        try:
            geometry = self.settings.value("geometry")
            if geometry:
                self.restoreGeometry(geometry)
        except BaseException as exc:
            print(f"Qt GUI: Could not restore geometry: {str(exc)}", file=sys.stderr)
        self.flowgraph_started = threading.Event()

        ##################################################
        # Variables
        ##################################################
        self.zmq_output_port = zmq_output_port = 20001
        self.zmq_input_port = zmq_input_port = 20002
        self.samp_rate = samp_rate = int(1E6)
        self.channel_noise_voltage = channel_noise_voltage = 0.0
        self.channel_gain_db = channel_gain_db = 0.0
        self.channel_doppler_hz = channel_doppler_hz = 0.0

        ##################################################
        # Blocks
        ##################################################

        self._channel_noise_voltage_range = qtgui.Range(0.0, 1.0, 0.01, 0.0, 200)
        self._channel_noise_voltage_win = qtgui.RangeWidget(self._channel_noise_voltage_range, self.set_channel_noise_voltage, "Noise Voltage", "counter_slider", float, QtCore.Qt.Horizontal)
        self.top_grid_layout.addWidget(self._channel_noise_voltage_win, 0, 1, 1, 1)
        for r in range(0, 1):
            self.top_grid_layout.setRowStretch(r, 1)
        for c in range(1, 2):
            self.top_grid_layout.setColumnStretch(c, 1)
        self._channel_gain_db_range = qtgui.Range(-60.0, 10.0, 0.5, 0.0, 200)
        self._channel_gain_db_win = qtgui.RangeWidget(self._channel_gain_db_range, self.set_channel_gain_db, "Channel Gain (dB)", "counter_slider", float, QtCore.Qt.Horizontal)
        self.top_grid_layout.addWidget(self._channel_gain_db_win, 0, 0, 1, 1)
        for r in range(0, 1):
            self.top_grid_layout.setRowStretch(r, 1)
        for c in range(0, 1):
            self.top_grid_layout.setColumnStretch(c, 1)
        self._channel_doppler_hz_range = qtgui.Range(-samp_rate*0.5, samp_rate*0.5, 1, 0.0, 200)
        self._channel_doppler_hz_win = qtgui.RangeWidget(self._channel_doppler_hz_range, self.set_channel_doppler_hz, "Channel Doppler (Hz)", "counter_slider", float, QtCore.Qt.Horizontal)
        self.top_grid_layout.addWidget(self._channel_doppler_hz_win, 0, 2, 1, 1)
        for r in range(0, 1):
            self.top_grid_layout.setRowStretch(r, 1)
        for c in range(2, 3):
            self.top_grid_layout.setColumnStretch(c, 1)
        self.zeromq_push_sink_0 = zeromq.push_sink(gr.sizeof_gr_complex, 1, f"tcp://127.0.0.1:{zmq_output_port}", 100, False, (-1), True)
        self.zeromq_pull_source_0 = zeromq.pull_source(gr.sizeof_gr_complex, 1, f"tcp://127.0.0.1:{zmq_input_port}", 100, False, (-1), True)
        self.qtgui_sink_x_0 = qtgui.sink_c(
            1024, #fftsize
            window.WIN_BLACKMAN_hARRIS, #wintype
            0.0, #fc
            samp_rate, #bw
            "Simple ZMQ Loopback", #name
            True, #plotfreq
            True, #plotwaterfall
            True, #plottime
            True, #plotconst
            None # parent
        )
        self.qtgui_sink_x_0.set_update_time(1.0/10)
        self._qtgui_sink_x_0_win = sip.wrapinstance(self.qtgui_sink_x_0.qwidget(), Qt.QWidget)

        self.qtgui_sink_x_0.enable_rf_freq(True)

        self.top_grid_layout.addWidget(self._qtgui_sink_x_0_win, 1, 0, 4, 2)
        for r in range(1, 5):
            self.top_grid_layout.setRowStretch(r, 1)
        for c in range(0, 2):
            self.top_grid_layout.setColumnStretch(c, 1)
        self.blocks_multiply_const_xx_0 = blocks.multiply_const_cc(10**(channel_gain_db/20.0), 1)
        self.blocks_freqshift_cc_0 = blocks.rotator_cc(2.0*math.pi*channel_doppler_hz/samp_rate)
        self.blocks_add_xx_0 = blocks.add_vcc(1)
        self.analog_noise_source_x_0 = analog.noise_source_c(analog.GR_GAUSSIAN, channel_noise_voltage, 0)


        ##################################################
        # Connections
        ##################################################
        self.connect((self.analog_noise_source_x_0, 0), (self.blocks_add_xx_0, 1))
        self.connect((self.blocks_add_xx_0, 0), (self.qtgui_sink_x_0, 0))
        self.connect((self.blocks_add_xx_0, 0), (self.zeromq_push_sink_0, 0))
        self.connect((self.blocks_freqshift_cc_0, 0), (self.blocks_add_xx_0, 0))
        self.connect((self.blocks_multiply_const_xx_0, 0), (self.blocks_freqshift_cc_0, 0))
        self.connect((self.zeromq_pull_source_0, 0), (self.blocks_multiply_const_xx_0, 0))


    def closeEvent(self, event):
        self.settings = Qt.QSettings("gnuradio/flowgraphs", "simple_zmq_loopback")
        self.settings.setValue("geometry", self.saveGeometry())
        self.stop()
        self.wait()

        event.accept()

    def get_zmq_output_port(self):
        return self.zmq_output_port

    def set_zmq_output_port(self, zmq_output_port):
        self.zmq_output_port = zmq_output_port

    def get_zmq_input_port(self):
        return self.zmq_input_port

    def set_zmq_input_port(self, zmq_input_port):
        self.zmq_input_port = zmq_input_port

    def get_samp_rate(self):
        return self.samp_rate

    def set_samp_rate(self, samp_rate):
        self.samp_rate = samp_rate
        self.blocks_freqshift_cc_0.set_phase_inc(2.0*math.pi*self.channel_doppler_hz/self.samp_rate)
        self.qtgui_sink_x_0.set_frequency_range(0.0, self.samp_rate)

    def get_channel_noise_voltage(self):
        return self.channel_noise_voltage

    def set_channel_noise_voltage(self, channel_noise_voltage):
        self.channel_noise_voltage = channel_noise_voltage
        self.analog_noise_source_x_0.set_amplitude(self.channel_noise_voltage)

    def get_channel_gain_db(self):
        return self.channel_gain_db

    def set_channel_gain_db(self, channel_gain_db):
        self.channel_gain_db = channel_gain_db
        self.blocks_multiply_const_xx_0.set_k(10**(self.channel_gain_db/20.0))

    def get_channel_doppler_hz(self):
        return self.channel_doppler_hz

    def set_channel_doppler_hz(self, channel_doppler_hz):
        self.channel_doppler_hz = channel_doppler_hz
        self.blocks_freqshift_cc_0.set_phase_inc(2.0*math.pi*self.channel_doppler_hz/self.samp_rate)




def main(top_block_cls=simple_zmq_loopback, options=None):

    qapp = Qt.QApplication(sys.argv)

    tb = top_block_cls()

    tb.start()
    tb.flowgraph_started.set()

    tb.show()

    def sig_handler(sig=None, frame=None):
        tb.stop()
        tb.wait()

        Qt.QApplication.quit()

    signal.signal(signal.SIGINT, sig_handler)
    signal.signal(signal.SIGTERM, sig_handler)

    timer = Qt.QTimer()
    timer.start(500)
    timer.timeout.connect(lambda: None)

    qapp.exec_()

if __name__ == '__main__':
    main()
